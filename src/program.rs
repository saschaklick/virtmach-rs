pub struct Program <'a> {
    pub source: u8,
    pub id: &'a str,
    pub data: &'a [u8],    
}

impl Program <'_> {
    pub const EMPTY: Program <'static> = Program { source: 255, id: "-empty-", data: &[] };
    pub const ERROR: Program <'static> = Program { source: 255, id: "-error-", data: &[] };
}