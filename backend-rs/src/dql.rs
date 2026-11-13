//! DQL subset parser + planner for `POST /api/nodes/query` (issue #5).
//!
//! ```text
//! TABLE name, updated_timestamp    -- or: LIST
//! FROM "Projects", #active         -- folders (quoted paths, subtree) and tags, comma = AND
//! WHERE updated_timestamp > -7d    -- comparisons + contains()/startswith() + AND/OR/NOT
//! SORT updated_timestamp DESC      -- one field + direction
//! LIMIT 20                         -- hard cap 100
//! ```
//!
//! Safety: field/function whitelist, parameterized bindings (no string
//! interpolation of values), nesting depth cap, statement timeout at the
//! route layer.

use std::fmt::Write as _;

/// Top-level output shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    List,
    Table(Vec<String>),
}

/// One FROM source: folder subtree or tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Folder(String),
    Tag(String),
}

/// WHERE expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Eq(String, Value),
    Ne(String, Value),
    Gt(String, Value),
    Lt(String, Value),
    Gte(String, Value),
    Lte(String, Value),
    Contains(String, String),
    StartsWith(String, String),
}

/// Literal value (durations resolve against now_ms at plan time).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    DurationMs(i64),
}

/// Parsed query: output + sources + optional filter/sort/limit.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub output: Output,
    pub sources: Vec<Source>,
    pub filter: Option<Expr>,
    pub sort: Option<(String, bool)>,
    pub limit: i64,
}

/// Built-in queryable columns (frontend names → SQL). `metadata.*` handled
/// separately (dynamic JSONB key, validated identifier charset).
pub fn builtin_column(field: &str) -> Option<&'static str> {
    Some(match field {
        "name" => "n.name",
        "tags" => "n.tags",
        "description" => "n.description",
        "role" => "n.role",
        "created_timestamp" => "n.created_timestamp",
        "updated_timestamp" => "n.updated_timestamp",
        _ => return None,
    })
}

/// TABLE column projection (LIST uses the fixed row shape).
pub fn table_column(field: &str) -> Option<String> {
    if let Some(col) = builtin_column(field) {
        return Some(col.to_string());
    }
    if let Some(key) = field.strip_prefix("metadata.")
        && is_ident(key) {
            return Some(format!("n.metadata->>'{key}'"));
        }
    None
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

fn check_field(field: &str) -> Result<(), String> {
    if builtin_column(field).is_some() {
        return Ok(());
    }
    if let Some(key) = field.strip_prefix("metadata.")
        && is_ident(key) {
            return Ok(());
        }
    Err(format!("unknown field '{field}'"))
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Str(String),
    Num(f64),
    Duration(i64),
    Tag(String),
    Eq,
    Ne,
    Gt,
    Lt,
    Gte,
    Lte,
    Comma,
    LParen,
    RParen,
}

fn lex_duration(s: &str) -> Option<(i64, usize)> {
    // -7d / -1M / -12h / -30m / -45s (M = 30 days, month-ish like Dataview).
    let b = s.as_bytes();
    if b.first() != Some(&b'-') {
        return None;
    }
    let mut i = 1;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == 1 || i >= b.len() {
        return None;
    }
    let n: i64 = s[1..i].parse().ok()?;
    let mult = match b[i] {
        b's' => 1_000,
        b'm' => 60_000,
        b'h' => 3_600_000,
        b'd' => 86_400_000,
        b'w' => 604_800_000,
        b'M' => 2_592_000_000,
        _ => return None,
    };
    Some((n * mult, i + 1))
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            '=' => {
                toks.push(Tok::Eq);
                i += 1;
            }
            '!' if b.get(i + 1) == Some(&b'=') => {
                toks.push(Tok::Ne);
                i += 2;
            }
            '>' if b.get(i + 1) == Some(&b'=') => {
                toks.push(Tok::Gte);
                i += 2;
            }
            '<' if b.get(i + 1) == Some(&b'=') => {
                toks.push(Tok::Lte);
                i += 2;
            }
            '>' => {
                toks.push(Tok::Gt);
                i += 1;
            }
            '<' => {
                toks.push(Tok::Lt);
                i += 1;
            }
            '"' => {
                let mut j = i + 1;
                let mut s = String::new();
                while j < b.len() && b[j] != b'"' {
                    if b[j] == b'\\' && j + 1 < b.len() {
                        s.push(b[j + 1] as char);
                        j += 2;
                    } else {
                        s.push(b[j] as char);
                        j += 1;
                    }
                }
                if j >= b.len() {
                    return Err("unterminated string".into());
                }
                toks.push(Tok::Str(s));
                i = j + 1;
            }
            '#' => {
                let mut j = i + 1;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'-' || b[j] == b'/')
                {
                    j += 1;
                }
                if j == i + 1 {
                    return Err("empty #tag".into());
                }
                toks.push(Tok::Tag(src[i + 1..j].to_string()));
                i = j;
            }
            '-' if b.get(i + 1).is_some_and(|d| d.is_ascii_digit()) => {
                if let Some((ms, len)) = lex_duration(&src[i..]) {
                    toks.push(Tok::Duration(ms));
                    i += len;
                } else {
                    return Err(format!("bad duration near '{}'", &src[i..(i + 8).min(src.len())]));
                }
            }
            _ if c.is_ascii_digit() || (c == '.' && b.get(i + 1).is_some_and(|d| d.is_ascii_digit())) => {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_digit() || b[j] == b'.') {
                    j += 1;
                }
                let n: f64 = src[i..j].parse().map_err(|_| format!("bad number '{}'", &src[i..j]))?;
                toks.push(Tok::Num(n));
                i = j;
            }
            _ if c.is_ascii_alphabetic() || c == '_' => {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_' || b[j] == b'.') {
                    j += 1;
                }
                toks.push(Tok::Word(src[i..j].to_string()));
                i = j;
            }
            _ => return Err(format!("unexpected character '{c}'")),
        }
    }
    Ok(toks)
}

// ---------------------------------------------------------------------------
// Parser (clause splitter + WHERE expression)
// ---------------------------------------------------------------------------

const MAX_EXPR_DEPTH: usize = 32;

fn word_is(t: &Tok, kw: &str) -> bool {
    matches!(t, Tok::Word(w) if w.eq_ignore_ascii_case(kw))
}

struct Cursor<'a> {
    toks: &'a [Tok],
    pos: usize,
    depth: usize,
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<&'a Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<&'a Tok> {
        let t = self.toks.get(self.pos)?;
        self.pos += 1;
        Some(t)
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(t) if word_is(t, "or")) {
            self.next();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_not()?;
        while matches!(self.peek(), Some(t) if word_is(t, "and")) {
            self.next();
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn parse_not(&mut self) -> Result<Expr, String> {
        self.depth += 1;
        if self.depth > MAX_EXPR_DEPTH {
            return Err("expression too deeply nested".into());
        }
        let out = if matches!(self.peek(), Some(t) if word_is(t, "not")) {
            self.next();
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_primary()
        };
        self.depth -= 1;
        out
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        self.depth += 1;
        if self.depth > MAX_EXPR_DEPTH {
            return Err("expression too deeply nested".into());
        }
        let out = match self.peek() {
            Some(Tok::LParen) => {
                self.next();
                let e = self.parse_or()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => Err("expected ')'".into()),
                }
            }
            Some(Tok::Word(w)) if w.eq_ignore_ascii_case("contains") || w.eq_ignore_ascii_case("startswith") => {
                let func = w.to_ascii_lowercase();
                self.next();
                if !matches!(self.next(), Some(Tok::LParen)) {
                    return Err(format!("expected '(' after {func}"));
                }
                let field = match self.next() {
                    Some(Tok::Word(f)) => f.clone(),
                    _ => return Err(format!("expected field in {func}()")),
                };
                if !matches!(self.next(), Some(Tok::Comma)) {
                    return Err(format!("expected ',' in {func}()"));
                }
                let arg = match self.next() {
                    Some(Tok::Str(s)) => s.clone(),
                    Some(Tok::Word(w)) => w.clone(),
                    _ => return Err(format!("expected string in {func}()")),
                };
                if !matches!(self.next(), Some(Tok::RParen)) {
                    return Err(format!("expected ')' in {func}()"));
                }
                check_field(&field)?;
                if func == "contains" {
                    Ok(Expr::Contains(field, arg))
                } else {
                    Ok(Expr::StartsWith(field, arg))
                }
            }
            Some(Tok::Word(f)) => {
                let field = f.clone();
                self.next();
                let op = match self.next() {
                    Some(Tok::Eq) => 0,
                    Some(Tok::Ne) => 1,
                    Some(Tok::Gt) => 2,
                    Some(Tok::Lt) => 3,
                    Some(Tok::Gte) => 4,
                    Some(Tok::Lte) => 5,
                    _ => return Err(format!("expected comparison after '{field}'")),
                };
                let value = match self.next() {
                    Some(Tok::Str(s)) => Value::Str(s.clone()),
                    Some(Tok::Word(w)) => Value::Str(w.clone()),
                    Some(Tok::Num(n)) => Value::Num(*n),
                    Some(Tok::Duration(ms)) => Value::DurationMs(*ms),
                    _ => return Err(format!("expected value after '{field}'")),
                };
                check_field(&field)?;
                Ok(match op {
                    0 => Expr::Eq(field, value),
                    1 => Expr::Ne(field, value),
                    2 => Expr::Gt(field, value),
                    3 => Expr::Lt(field, value),
                    4 => Expr::Gte(field, value),
                    _ => Expr::Lte(field, value),
                })
            }
            other => Err(format!("unexpected token {other:?} in WHERE")),
        };
        self.depth -= 1;
        out
    }
}

/// Split top-level clauses: TABLE/LIST ... FROM ... WHERE ... SORT ... LIMIT.
pub fn parse(src: &str) -> Result<Query, String> {
    let toks = lex(src)?;
    if toks.is_empty() {
        return Err("empty query".into());
    }
    let mut i = 0;
    // Output clause.
    let output = match toks.get(i) {
        Some(t) if word_is(t, "list") => {
            i += 1;
            Output::List
        }
        Some(t) if word_is(t, "table") => {
            i += 1;
            let mut cols = Vec::new();
            loop {
                match toks.get(i) {
                    Some(Tok::Word(c)) => {
                        if table_column(c).is_none() {
                            return Err(format!("unknown column '{c}'"));
                        }
                        cols.push(c.clone());
                        i += 1;
                    }
                    _ => return Err("expected column after TABLE".into()),
                }
                match toks.get(i) {
                    Some(Tok::Comma) => {
                        i += 1;
                    }
                    _ => break,
                }
            }
            if cols.is_empty() {
                return Err("TABLE needs at least one column".into());
            }
            Output::Table(cols)
        }
        other => return Err(format!("query must start with LIST or TABLE, got {other:?}")),
    };
    // Optional FROM.
    let mut sources = Vec::new();
    if matches!(toks.get(i), Some(t) if word_is(t, "from")) {
        i += 1;
        loop {
            match toks.get(i) {
                Some(Tok::Str(p)) => {
                    sources.push(Source::Folder(p.clone()));
                    i += 1;
                }
                Some(Tok::Tag(t)) => {
                    sources.push(Source::Tag(t.clone()));
                    i += 1;
                }
                _ => return Err("expected \"folder\" or #tag after FROM".into()),
            }
            match toks.get(i) {
                Some(Tok::Comma) => {
                    i += 1;
                }
                _ => break,
            }
        }
    }
    // Optional WHERE (runs to SORT/LIMIT/end).
    let mut filter = None;
    if matches!(toks.get(i), Some(t) if word_is(t, "where")) {
        i += 1;
        let mut j = i;
        while j < toks.len()
            && !matches!(&toks[j], t if word_is(t, "sort") || word_is(t, "limit"))
        {
            j += 1;
        }
        if j == i {
            return Err("empty WHERE".into());
        }
        let mut cur = Cursor { toks: &toks[i..j], pos: 0, depth: 0 };
        filter = Some(cur.parse_or()?);
        if cur.pos != j - i {
            return Err("trailing tokens in WHERE".into());
        }
        i = j;
    }
    // Optional SORT.
    let mut sort = None;
    if matches!(toks.get(i), Some(t) if word_is(t, "sort")) {
        i += 1;
        let field = match toks.get(i) {
            Some(Tok::Word(f)) => {
                check_field(f)?;
                f.clone()
            }
            _ => return Err("expected field after SORT".into()),
        };
        i += 1;
        let desc = matches!(toks.get(i), Some(t) if word_is(t, "desc"));
        if desc
            || matches!(toks.get(i), Some(t) if word_is(t, "asc"))
        {
            i += 1;
        }
        sort = Some((field, desc));
    }
    // Optional LIMIT.
    let mut limit = 100i64;
    if matches!(toks.get(i), Some(t) if word_is(t, "limit")) {
        i += 1;
        match toks.get(i) {
            Some(Tok::Num(n)) if *n >= 1.0 => {
                limit = (*n as i64).min(100);
                i += 1;
            }
            _ => return Err("LIMIT needs a positive number".into()),
        }
    }
    if i != toks.len() {
        return Err(format!("unexpected trailing tokens: {:?}", &toks[i..]));
    }
    Ok(Query { output, sources, filter, sort, limit })
}

// ---------------------------------------------------------------------------
// Planner: Query → parameterized SQL (sqlx $N bindings via BoundValue list)
// ---------------------------------------------------------------------------

/// One bound parameter (sqlx binds by position).
#[derive(Debug, Clone, PartialEq)]
pub enum BoundValue {
    Str(String),
    I64(i64),
    F64(f64),
}

/// Planned statement: SQL text with $N placeholders + ordered bindings.
#[derive(Debug, Clone)]
pub struct Planned {
    pub sql: String,
    pub bindings: Vec<BoundValue>,
}

fn sql_field(field: &str) -> String {
    if let Some(col) = builtin_column(field) {
        return col.to_string();
    }
    // Validated by check_field at parse time.
    let key = field.strip_prefix("metadata.").unwrap_or(field);
    format!("n.metadata->>'{key}'")
}

fn sql_cmp(field: &str, op: &str, v: &Value, now_ms: i64, next: &mut usize, out: &mut Vec<BoundValue>) -> String {
    let col = sql_field(field);
    let is_ts = field == "created_timestamp" || field == "updated_timestamp";
    let is_num_col = is_ts || field == "role";
    match v {
        Value::DurationMs(ms) => {
            *next += 1;
            out.push(BoundValue::I64(now_ms - ms));
            format!("{col} {op} ${}", *next)
        }
        Value::Num(n) => {
            *next += 1;
            if is_num_col {
                out.push(BoundValue::I64(*n as i64));
            } else {
                out.push(BoundValue::F64(*n));
            }
            format!("{col} {op} ${}", *next)
        }
        Value::Str(s) => {
            // Timestamps also accept plain numbers as strings ("1794000000000").
            if is_num_col
                && let Ok(n) = s.parse::<i64>() {
                    *next += 1;
                    out.push(BoundValue::I64(n));
                    return format!("{col} {op} ${}", *next);
                }
            *next += 1;
            out.push(BoundValue::Str(s.clone()));
            format!("{col} {op} ${}", *next)
        }
    }
}

fn sql_expr(e: &Expr, now_ms: i64, next: &mut usize, out: &mut Vec<BoundValue>) -> String {
    match e {
        Expr::And(a, b) => {
            format!("({} AND {})", sql_expr(a, now_ms, next, out), sql_expr(b, now_ms, next, out))
        }
        Expr::Or(a, b) => {
            format!("({} OR {})", sql_expr(a, now_ms, next, out), sql_expr(b, now_ms, next, out))
        }
        Expr::Not(a) => format!("(NOT {})", sql_expr(a, now_ms, next, out)),
        Expr::Eq(f, v) => sql_cmp(f, "=", v, now_ms, next, out),
        Expr::Ne(f, v) => sql_cmp(f, "<>", v, now_ms, next, out),
        Expr::Gt(f, v) => sql_cmp(f, ">", v, now_ms, next, out),
        Expr::Lt(f, v) => sql_cmp(f, "<", v, now_ms, next, out),
        Expr::Gte(f, v) => sql_cmp(f, ">=", v, now_ms, next, out),
        Expr::Lte(f, v) => sql_cmp(f, "<=", v, now_ms, next, out),
        Expr::Contains(f, s) => {
            *next += 1;
            out.push(BoundValue::Str(format!("%{s}%")));
            format!("{} LIKE ${}", sql_field(f), *next)
        }
        Expr::StartsWith(f, s) => {
            *next += 1;
            out.push(BoundValue::Str(format!("{s}%")));
            format!("{} LIKE ${}", sql_field(f), *next)
        }
    }
}

/// Plan a query. `$1` = user id (bound by the caller); `$2..` = `bindings`
/// in order. `folders` maps each `FROM "path"` to its resolved folder id
/// (`None` = path not found → `AND false`, empty result).
pub fn plan(
    q: &Query,
    folders: &std::collections::HashMap<String, Option<uuid::Uuid>>,
    now_ms: i64,
) -> Planned {
    let mut bindings: Vec<BoundValue> = Vec::new();
    let mut next = 1usize; // $1 reserved for user id
    let mut sql = String::from(
        "SELECT n.id, n.user_id, n.parent_id, n.name, n.description, n.tags, n.role, n.color, n.icon, n.theme, n.accessibility, n.access, n.display, n.\"order\" AS node_order, n.size, n.metadata, n.created_timestamp, n.updated_timestamp, CASE WHEN n.role IN (1, 2) THEN n.thumbnail ELSE NULL END AS thumbnail FROM nodes n WHERE n.user_id = $1 AND n.deleted_at IS NULL",
    );
    // Sources (comma = AND): folders by subtree, tags by membership.
    for src in &q.sources {
        match src {
            Source::Folder(path) => match folders.get(path).copied().flatten() {
                Some(id) => {
                    next += 1;
                    bindings.push(BoundValue::Str(id.to_string()));
                    let _ = write!(
                        sql,
                        " AND n.id IN (WITH RECURSIVE sub(id) AS (SELECT ${next}::uuid UNION ALL SELECT c.id FROM nodes c JOIN sub s ON c.parent_id = s.id WHERE c.deleted_at IS NULL) SELECT id FROM sub)"
                    );
                }
                None => sql.push_str(" AND false"),
            },
            Source::Tag(tag) => {
                next += 1;
                bindings.push(BoundValue::Str(format!("%,{tag},%")));
                let _ = write!(sql, " AND (',' || REPLACE(COALESCE(n.tags,''), ' ', '') || ',') LIKE ${next}");
            }
        }
    }
    if let Some(f) = &q.filter {
        let cond = sql_expr(f, now_ms, &mut next, &mut bindings);
        let _ = write!(sql, " AND {cond}");
    }
    if let Some((field, desc)) = &q.sort {
        let col = sql_field(field);
        let _ = write!(sql, " ORDER BY {col} {}", if *desc { "DESC" } else { "ASC" });
    } else {
        sql.push_str(" ORDER BY n.updated_timestamp DESC");
    }
    next += 1;
    bindings.push(BoundValue::I64(q.limit));
    let _ = write!(sql, " LIMIT ${next}");
    Planned { sql, bindings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn no_folders() -> HashMap<String, Option<uuid::Uuid>> {
        HashMap::new()
    }

    #[test]
    fn list_from_tag_with_duration() {
        let q = parse("LIST FROM #active WHERE updated_timestamp > -7d SORT updated_timestamp DESC LIMIT 20").unwrap();
        assert_eq!(q.output, Output::List);
        assert_eq!(q.sources, vec![Source::Tag("active".into())]);
        assert_eq!(q.limit, 20);
        assert_eq!(q.sort, Some(("updated_timestamp".into(), true)));
        let p = plan(&q, &no_folders(), 1_000_000_000_000);
        assert!(p.sql.contains("COALESCE(n.tags,'')"), "{}", p.sql);
        assert!(p.sql.contains("n.updated_timestamp > $"), "{}", p.sql);
        assert_eq!(p.bindings.len(), 3); // tag + cutoff + limit
        assert_eq!(p.bindings[1], BoundValue::I64(1_000_000_000_000 - 7 * 86_400_000));
    }

    #[test]
    fn table_multi_source_and_or() {
        let q = parse("TABLE name, metadata.status FROM \"Projects\", #active WHERE contains(name, \"plan\") OR metadata.status = \"todo\"").unwrap();
        assert_eq!(q.output, Output::Table(vec!["name".into(), "metadata.status".into()]));
        assert_eq!(q.sources.len(), 2);
        let mut folders = HashMap::new();
        let id = uuid::Uuid::now_v7();
        folders.insert("Projects".into(), Some(id));
        let p = plan(&q, &folders, 0);
        assert!(p.sql.contains("WITH RECURSIVE sub"), "{}", p.sql);
        assert!(p.sql.contains("n.metadata->>'status'"), "{}", p.sql);
        assert!(p.sql.contains(" OR "), "{}", p.sql);
        assert!(p.bindings.iter().any(|b| *b == BoundValue::Str(id.to_string())));
    }

    #[test]
    fn missing_folder_yields_false() {
        let q = parse("LIST FROM \"Nope\"").unwrap();
        let p = plan(&q, &no_folders(), 0);
        assert!(p.sql.contains("AND false"), "{}", p.sql);
    }

    #[test]
    fn limit_hard_cap() {
        let q = parse("LIST LIMIT 9999").unwrap();
        assert_eq!(q.limit, 100);
    }

    #[test]
    fn rejects_unknown_field_and_injection() {
        assert!(parse("LIST WHERE password = \"x\"").is_err());
        assert!(parse("TABLE name; DROP TABLE nodes").is_err());
        assert!(parse("LIST WHERE name = \"x\" OR 1=1").is_err());
        assert!(parse("LIST WHERE metadata.bad-key = 1").is_err());
    }

    #[test]
    fn nesting_cap() {
        let deep = "LIST WHERE ".to_string() + &"NOT ".repeat(40) + "name = \"x\"";
        assert!(parse(&deep).is_err());
    }
}
