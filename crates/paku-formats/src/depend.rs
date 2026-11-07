// SPDX-License-Identifier: MIT

//! Dependency strings such as `foo`, `foo>=1.2-1` or `foo: optional feature`.

use std::fmt;

/// Version comparison operator in a dependency string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepOp {
    /// `=`
    Eq,
    /// `>=`
    Ge,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `<`
    Lt,
}

impl DepOp {
    /// Operator text as written in dependency strings.
    pub fn as_str(self) -> &'static str {
        match self {
            DepOp::Eq => "=",
            DepOp::Ge => ">=",
            DepOp::Le => "<=",
            DepOp::Gt => ">",
            DepOp::Lt => "<",
        }
    }
}

/// A parsed dependency, provision or conflict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Depend {
    /// Package (or virtual provision) name.
    pub name: String,
    /// Version constraint, if any.
    pub constraint: Option<(DepOp, String)>,
    /// Description after `: ` (optional dependencies only).
    pub desc: Option<String>,
}

impl Depend {
    /// Parses a dependency string. Never fails: anything unrecognized is a bare name.
    pub fn parse(s: &str) -> Self {
        let (spec, desc) = match s.split_once(": ") {
            Some((spec, desc)) => (spec, Some(desc.to_string())),
            None => (s, None),
        };
        let Some(pos) = spec.find(['<', '>', '=']) else {
            return Depend {
                name: spec.to_string(),
                constraint: None,
                desc,
            };
        };
        let (name, rest) = spec.split_at(pos);
        let (op, version) = split_op(rest);
        Depend {
            name: name.to_string(),
            constraint: Some((op, version.to_string())),
            desc,
        }
    }
}

fn split_op(rest: &str) -> (DepOp, &str) {
    for (text, op) in [
        (">=", DepOp::Ge),
        ("<=", DepOp::Le),
        ("=", DepOp::Eq),
        (">", DepOp::Gt),
        ("<", DepOp::Lt),
    ] {
        if let Some(v) = rest.strip_prefix(text) {
            return (op, v);
        }
    }
    (DepOp::Eq, rest)
}

impl fmt::Display for Depend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if let Some((op, v)) = &self.constraint {
            write!(f, "{}{}", op.as_str(), v)?;
        }
        if let Some(d) = &self.desc {
            write!(f, ": {d}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_name() {
        let d = Depend::parse("mingw-w64-ucrt-x86_64-gcc-libs");
        assert_eq!(d.name, "mingw-w64-ucrt-x86_64-gcc-libs");
        assert_eq!(d.constraint, None);
    }

    #[test]
    fn operators() {
        let cases = [
            ("a>=1", DepOp::Ge),
            ("a<=1", DepOp::Le),
            ("a=1", DepOp::Eq),
            ("a>1", DepOp::Gt),
            ("a<1", DepOp::Lt),
        ];
        for (s, op) in cases {
            let d = Depend::parse(s);
            assert_eq!(d.name, "a");
            assert_eq!(d.constraint, Some((op, "1".to_string())), "{s}");
            assert_eq!(d.to_string(), s);
        }
    }

    #[test]
    fn optdepend_description() {
        let d = Depend::parse("python: for scripts");
        assert_eq!(d.name, "python");
        assert_eq!(d.desc.as_deref(), Some("for scripts"));
        assert_eq!(d.to_string(), "python: for scripts");
    }
}
