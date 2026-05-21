pub struct CreateModifiers {
    pub or_replace: bool,
    pub temporary: bool,
    pub unlogged: bool,
    pub unique: bool,
    pub materialized: bool,
}