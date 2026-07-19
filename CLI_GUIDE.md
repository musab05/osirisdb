# OsirisDB CLI Statement Reference Guide

OsirisDB is a custom SQL database engine. The command-line interface supports the following statements, each of which must end with a semicolon (`;`).

---

## 1. Database Operations

### CREATE DATABASE
Creates a new database namespace and sets up its physical storage directory.
```sql
-- Minimal syntax
CREATE DATABASE my_new_db;

-- Full syntax with options (IF NOT EXISTS, OWNER, ENCODING, CONNECTION LIMIT)
CREATE DATABASE IF NOT EXISTS my_new_db OWNER postgres ENCODING 'UTF8' CONNECTION LIMIT 100;
```

### USE
Switches the active database context for the current session.
```sql
USE my_new_db;
```

---

## 2. Schema Operations

### CREATE SCHEMA
Creates a namespace schema inside the active database context.
```sql
CREATE SCHEMA myschema;
```

---

## 3. Table Operations

### CREATE TABLE
Creates a new table inside a schema. Supports primary keys and constraint definitions.
```sql
CREATE TABLE myschema.users (
    id INT PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE
);
```

---

## 4. Data Operations

### INSERT INTO
Inserts data rows into a table.
```sql
-- Insert individual rows
INSERT INTO myschema.users (id, name, email) VALUES (1, 'Alice', 'alice@example.com');
INSERT INTO myschema.users (id, name, email) VALUES (2, 'Bob', 'bob@example.com');
```

### SELECT
Queries data from a table, supporting projection lists, custom filtering conditions (`WHERE`), and operators.
```sql
-- Select all columns
SELECT * FROM myschema.users;

-- Filter with conditions
SELECT id, name FROM myschema.users WHERE id != 3;
```

---

## Session Control Commands
The terminal session supports the following keywords to exit the CLI:
* `\q`
* `exit`
* `quit`
