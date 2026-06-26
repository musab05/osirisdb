use crate::{
    ast::{DataType, Value},
    catalog::objects::ColumnEntry,
    common::interner::Interner,
    storage::error::StorageError,
};

// ─────────────────────────────────────────────────────────────────────────────
// On-disk tuple format
// ─────────────────────────────────────────────────────────────────────────────
//
// A serialized tuple is a flat byte sequence:
//
//   [ NULL bitmap | col_0 bytes | col_1 bytes | ... | col_n bytes ]
//
// NULL bitmap
// ───────────
//   ceil(column_count / 8) bytes.
//   Bit i (0-indexed, LSB-first within each byte) is SET if column i is NULL.
//   If a column is NULL, no bytes are written for its value — only the bitmap
//   records its presence. This keeps NULL storage cheap.
//
// Fixed-width column values (written in little-endian byte order)
// ────────────────────────────────────────────────────────────────
//   DataType::SmallInt  →  2 bytes  (i16)
//   DataType::Int       →  8 bytes  (i64)   ← we always use i64 internally
//   DataType::BigInt    →  8 bytes  (i64)
//   DataType::Boolean   →  1 byte   (0 = false, 1 = true)
//   DataType::Float     →  4 bytes  (f32 bits)
//   DataType::Double    →  8 bytes  (f64 bits)
//
// Variable-width column values
// ────────────────────────────
//   DataType::VarChar / Char / Text → 4-byte u32 length prefix + UTF-8 bytes
//
// Unsupported types (Decimal, Binary, Json, Date, Timestamp, UUID, Array,
// Custom, etc.) return StorageError::TupleError for now — they can be added
// incrementally as needed.
// ─────────────────────────────────────────────────────────────────────────────

/// Serializes a row of `values` into a flat byte buffer according to `schema`.
///
/// # Errors
///
/// - [`StorageError::TupleError`] if `values.len() != schema.len()`
/// - [`StorageError::TupleError`] if a value's type does not match its
///   column's declared [`DataType`]
/// - [`StorageError::TupleError`] if a non-nullable column receives `NULL`
/// - [`StorageError::TupleError`] for unsupported data types
pub fn serialize_tuple(
    schema: &[ColumnEntry],
    values: &[Value],
    interner: &Interner,
) -> Result<Vec<u8>, StorageError> {
    if values.len() != schema.len() {
        return Err(StorageError::TupleError(format!(
            "expected {} values for {} columns",
            schema.len(),
            values.len(),
        )));
    }

    let col_count = schema.len();

    // ── NULL bitmap ───────────────────────────────────────────────────────────
    // ceil(col_count / 8) bytes, one bit per column.
    let bitmap_bytes = bitmap_size(col_count);
    let mut buf: Vec<u8> = vec![0u8; bitmap_bytes];

    // ── Encode each value ─────────────────────────────────────────────────────
    for (i, (col, val)) in schema.iter().zip(values.iter()).enumerate() {
        match val {
            Value::Null => {
                // Validate the column allows NULL.
                if !col.nullable {
                    return Err(StorageError::TupleError(format!(
                        "column {:?} is NOT NULL but received NULL",
                        i
                    )));
                }
                // Set the NULL bit — no value bytes written.
                set_null_bit(&mut buf, i);
            }
            _ => {
                // Encode the value bytes and append to buf.
                encode_value(&col.data_type, val, interner, &mut buf)?;
            }
        }
    }

    Ok(buf)
}

/// Deserializes a flat byte buffer back into a row of [`Value`]s according
/// to `schema`.
///
/// # Errors
///
/// - [`StorageError::TupleError`] if the buffer is too short
/// - [`StorageError::TupleError`] for unsupported data types
pub fn deserialize_tuple(
    schema: &[ColumnEntry],
    data: &[u8],
    interner: &mut Interner,
) -> Result<Vec<Value>, StorageError> {
    let col_count = schema.len();
    let bitmap_bytes = bitmap_size(col_count);

    if data.len() < bitmap_bytes {
        return Err(StorageError::TupleError(
            "tuple data too short to contain NULL bitmap".into(),
        ));
    }

    let mut cursor = bitmap_bytes; // byte offset into `data`, past the bitmap
    let mut values = Vec::with_capacity(col_count);

    for (i, col) in schema.iter().enumerate() {
        if is_null(data, i) {
            values.push(Value::Null);
        } else {
            let (val, bytes_read) = decode_value(&col.data_type, &data[cursor..], interner)?;
            cursor += bytes_read;
            values.push(val);
        }
    }

    Ok(values)
}

// ─────────────────────────────────────────────────────────────────────────────
// NULL bitmap helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns the number of bytes needed to store `col_count` NULL bits.
///
/// Example: 9 columns → 2 bytes (bits 0-7 in byte 0, bit 8 in byte 1).
fn bitmap_size(col_count: usize) -> usize {
    // Integer ceiling division: (n + 7) / 8
    (col_count + 7) / 8
}

/// Sets the NULL bit for column `col_idx` in the bitmap at the start of `buf`.
///
/// Bit layout: column `i` lives in byte `i / 8`, at bit position `i % 8`
/// (LSB = column 0 within the byte).
fn set_null_bit(buf: &mut [u8], col_idx: usize) {
    let byte = col_idx / 8;
    let bit = col_idx % 8;
    buf[byte] |= 1 << bit;
}

/// Returns `true` if the NULL bit for column `col_idx` is set in `data`.
fn is_null(data: &[u8], col_idx: usize) -> bool {
    let byte = col_idx / 8;
    let bit = col_idx % 8;
    (data[byte] >> bit) & 1 == 1
}

// ─────────────────────────────────────────────────────────────────────────────
// Value encoding
// ─────────────────────────────────────────────────────────────────────────────

/// Appends the encoded bytes for `val` (of declared type `data_type`) to `buf`.
///
/// Returns an error if `val`'s runtime type does not match `data_type`, or if
/// the type is not yet supported.
fn encode_value(
    data_type: &DataType,
    val: &Value,
    interner: &Interner,
    buf: &mut Vec<u8>,
) -> Result<(), StorageError> {
    match (data_type, val) {
        // ── Integer types ─────────────────────────────────────────────
        (DataType::SmallInt, Value::Int(n)) => {
            // Store as i16 — validate range first.
            let n16 = i16::try_from(*n).map_err(|_| {
                StorageError::TupleError(format!("value {} out of range for SMALLINT", n))
            })?;
            buf.extend_from_slice(&n16.to_le_bytes());
        }
        (DataType::Int, Value::Int(n)) => {
            // INT maps to i64 internally (same as BIGINT in our Value enum).
            buf.extend_from_slice(&n.to_le_bytes());
        }
        (DataType::BigInt, Value::Int(n)) => {
            buf.extend_from_slice(&n.to_le_bytes());
        }

        // ── Boolean ───────────────────────────────────────────────────
        (DataType::Boolean, Value::Boolean(b)) => {
            buf.push(if *b { 1 } else { 0 });
        }

        // ── Floating point ────────────────────────────────────────────
        (DataType::Float, Value::Float(f)) => {
            buf.extend_from_slice(&(*f as f32).to_bits().to_le_bytes());
        }
        (DataType::Double, Value::Float(f)) => {
            buf.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        // Integer literal widened to float column — binder permits this.
        (DataType::Float, Value::Int(n)) => {
            buf.extend_from_slice(&(*n as f32).to_bits().to_le_bytes());
        }
        (DataType::Double, Value::Int(n)) => {
            buf.extend_from_slice(&(*n as f64).to_bits().to_le_bytes());
        }

        // ── Variable-length strings ───────────────────────────────────
        // VarChar, Char, and Text all serialize identically:
        //   4-byte u32 length prefix (byte count, not char count)
        //   + raw UTF-8 bytes
        (DataType::VarChar(_) | DataType::Char(_) | DataType::Text, Value::String(sym)) => {
            let s = interner.resolve(*sym);
            let bytes = s.as_bytes();
            let len = bytes.len() as u32;
            buf.extend_from_slice(&len.to_le_bytes()); // 4-byte length prefix
            buf.extend_from_slice(bytes); // UTF-8 payload
        }

        // ── Type mismatch ─────────────────────────────────────────────
        (expected, actual) => {
            return Err(StorageError::TupleError(format!(
                "type mismatch: column declared as {:?} but got {:?}",
                expected, actual
            )));
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Value decoding
// ─────────────────────────────────────────────────────────────────────────────

/// Reads one value of `data_type` from the start of `data`.
///
/// Returns `(value, bytes_consumed)` so the caller can advance the cursor.
fn decode_value(
    data_type: &DataType,
    data: &[u8],
    interner: &mut Interner,
) -> Result<(Value, usize), StorageError> {
    match data_type {
        // ── Integer types ─────────────────────────────────────────────
        DataType::SmallInt => {
            need(data, 2, "SMALLINT")?;
            let n = i16::from_le_bytes(data[0..2].try_into().unwrap());
            Ok((Value::Int(n as i64), 2))
        }
        DataType::Int | DataType::BigInt => {
            need(data, 8, "INT/BIGINT")?;
            let n = i64::from_le_bytes(data[0..8].try_into().unwrap());
            Ok((Value::Int(n), 8))
        }

        // ── Boolean ───────────────────────────────────────────────────
        DataType::Boolean => {
            need(data, 1, "BOOLEAN")?;
            Ok((Value::Boolean(data[0] != 0), 1))
        }

        // ── Floating point ────────────────────────────────────────────
        DataType::Float => {
            need(data, 4, "FLOAT")?;
            let bits = u32::from_le_bytes(data[0..4].try_into().unwrap());
            Ok((Value::Float(f32::from_bits(bits) as f64), 4))
        }
        DataType::Double => {
            need(data, 8, "DOUBLE")?;
            let bits = u64::from_le_bytes(data[0..8].try_into().unwrap());
            Ok((Value::Float(f64::from_bits(bits)), 8))
        }

        // ── Variable-length strings ───────────────────────────────────
        DataType::VarChar(_) | DataType::Char(_) | DataType::Text => {
            // Read 4-byte length prefix first.
            need(data, 4, "string length prefix")?;
            let len = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;

            // Then read `len` bytes of UTF-8 payload.
            need(&data[4..], len, "string payload")?;
            let s = str::from_utf8(&data[4..4 + len])
                .map_err(|_| StorageError::TupleError("invalid UTF-8 in stored string".into()))?;
            let sym = interner.intern(s);
            Ok((Value::String(sym), 4 + len))
        }

        // ── Unsupported ───────────────────────────────────────────────
        other => Err(StorageError::TupleError(format!(
            "unsupported data type for deserialization: {:?}",
            other
        ))),
    }
}

/// Returns an error if `data` has fewer than `n` bytes available.
fn need(data: &[u8], n: usize, ctx: &str) -> Result<(), StorageError> {
    if data.len() < n {
        Err(StorageError::TupleError(format!(
            "unexpected end of tuple data reading {}: need {} bytes, have {}",
            ctx,
            n,
            data.len()
        )))
    } else {
        Ok(())
    }
}
