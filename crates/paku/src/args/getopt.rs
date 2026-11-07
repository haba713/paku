// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (c) 2006-2024 Pacman Development Team <pacman-dev@lists.archlinux.org>

//! A `getopt_long`-compatible tokenizer, so paku accepts exactly the argument
//! shapes pacman does: clustered short flags (`-Syu`), attached or separate
//! short arguments (`-bdir`, `-b dir`), `--long=value`, unique long-option
//! prefixes (`--noconf`), options after targets, and `--` to end options.

/// Whether an option takes an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HasArg {
    /// Flag only.
    No,
    /// Argument required (`--root dir` or `--root=dir`).
    Required,
    /// Argument optional, only via `=` (`--debug=2`).
    Optional,
}

/// Identity of a parsed option: a short letter or a long name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// `-x`
    Short(char),
    /// `--name`
    Long(&'static str),
}

/// One option occurrence with its argument, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opt {
    /// Which option.
    pub key: Key,
    /// Its argument.
    pub value: Option<String>,
}

/// Tokenizer result: options in order, then the non-option targets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    /// Options in command-line order.
    pub opts: Vec<Opt>,
    /// Remaining arguments (targets).
    pub targets: Vec<String>,
}

/// Option table: short letters with their argument mode, and long options.
pub struct Spec {
    /// Short options.
    pub shorts: &'static [(char, HasArg)],
    /// Long options.
    pub longs: &'static [(&'static str, HasArg)],
}

impl Spec {
    /// Splits `args` (without the program name) into options and targets.
    /// Errors carry getopt-style messages.
    pub fn parse(&self, args: &[String]) -> Result<Parsed, String> {
        let mut parsed = Parsed::default();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            i += 1;
            if arg == "--" {
                parsed.targets.extend(args[i..].iter().cloned());
                break;
            }
            if let Some(body) = arg.strip_prefix("--") {
                parsed.opts.push(self.parse_long(body, args, &mut i)?);
            } else if arg.len() > 1 && arg.starts_with('-') {
                self.parse_shorts(&arg[1..], args, &mut i, &mut parsed.opts)?;
            } else {
                parsed.targets.push(arg.clone());
            }
        }
        Ok(parsed)
    }

    fn parse_long(&self, body: &str, args: &[String], i: &mut usize) -> Result<Opt, String> {
        let (name, inline) = match body.split_once('=') {
            Some((n, v)) => (n, Some(v.to_string())),
            None => (body, None),
        };
        let (long, has_arg) = self.find_long(name)?;
        let value = match (has_arg, inline) {
            (HasArg::No, Some(_)) => {
                return Err(format!("option '--{long}' doesn't allow an argument"));
            }
            (HasArg::Required, None) => Some(next_arg(args, i, &format!("option '--{long}'"))?),
            (_, v) => v,
        };
        Ok(Opt {
            key: Key::Long(long),
            value,
        })
    }

    /// Exact match first, then a unique prefix (getopt_long behaviour).
    fn find_long(&self, name: &str) -> Result<(&'static str, HasArg), String> {
        if let Some(&(n, a)) = self.longs.iter().find(|(n, _)| *n == name) {
            return Ok((n, a));
        }
        let matches: Vec<_> = self.longs.iter().filter(|(n, _)| n.starts_with(name)).collect();
        match matches.len() {
            1 if !name.is_empty() => Ok(*matches[0]),
            0 | 1 => Err(format!("unrecognized option '--{name}'")),
            _ => Err(format!("option '--{name}' is ambiguous")),
        }
    }

    fn parse_shorts(&self, cluster: &str, args: &[String], i: &mut usize, out: &mut Vec<Opt>) -> Result<(), String> {
        for (pos, c) in cluster.char_indices() {
            let Some(&(_, has_arg)) = self.shorts.iter().find(|(s, _)| *s == c) else {
                return Err(format!("invalid option -- '{c}'"));
            };
            if has_arg == HasArg::No {
                out.push(Opt {
                    key: Key::Short(c),
                    value: None,
                });
                continue;
            }
            let rest = &cluster[pos + c.len_utf8()..];
            let value = if rest.is_empty() {
                next_arg(args, i, &format!("option requires an argument -- '{c}'"))?
            } else {
                rest.to_string()
            };
            out.push(Opt {
                key: Key::Short(c),
                value: Some(value),
            });
            return Ok(());
        }
        Ok(())
    }
}

fn next_arg(args: &[String], i: &mut usize, what: &str) -> Result<String, String> {
    let Some(v) = args.get(*i) else {
        return Err(if what.starts_with("option '") {
            format!("{what} requires an argument")
        } else {
            what.to_string()
        });
    };
    *i += 1;
    Ok(v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: Spec = Spec {
        shorts: &[
            ('S', HasArg::No),
            ('y', HasArg::No),
            ('u', HasArg::No),
            ('b', HasArg::Required),
        ],
        longs: &[
            ("sync", HasArg::No),
            ("noconfirm", HasArg::No),
            ("noprogressbar", HasArg::No),
            ("root", HasArg::Required),
            ("debug", HasArg::Optional),
        ],
    };

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    fn keys(p: &Parsed) -> Vec<Key> {
        p.opts.iter().map(|o| o.key).collect()
    }

    #[test]
    fn clustered_shorts_and_targets_anywhere() {
        let p = SPEC.parse(&args("foo -Syu bar --noconfirm")).unwrap();
        assert_eq!(
            keys(&p),
            [
                Key::Short('S'),
                Key::Short('y'),
                Key::Short('u'),
                Key::Long("noconfirm")
            ]
        );
        assert_eq!(p.targets, ["foo", "bar"]);
    }

    #[test]
    fn short_arguments() {
        let p = SPEC.parse(&args("-Sbdir")).unwrap();
        assert_eq!(p.opts[1].value.as_deref(), Some("dir"));
        let p = SPEC.parse(&args("-Sb dir x")).unwrap();
        assert_eq!(p.opts[1].value.as_deref(), Some("dir"));
        assert_eq!(p.targets, ["x"]);
        assert!(SPEC.parse(&args("-Sb")).is_err());
    }

    #[test]
    fn long_arguments_and_prefixes() {
        let p = SPEC
            .parse(&args("--root=/x --root /y --debug --debug=2 --noconf"))
            .unwrap();
        let values: Vec<_> = p.opts.iter().map(|o| o.value.as_deref()).collect();
        assert_eq!(values, [Some("/x"), Some("/y"), None, Some("2"), None]);
        assert_eq!(p.opts[4].key, Key::Long("noconfirm"));
    }

    #[test]
    fn errors_match_getopt() {
        assert_eq!(SPEC.parse(&args("-Z")).unwrap_err(), "invalid option -- 'Z'");
        assert_eq!(
            SPEC.parse(&args("--bogus")).unwrap_err(),
            "unrecognized option '--bogus'"
        );
        assert_eq!(SPEC.parse(&args("--no")).unwrap_err(), "option '--no' is ambiguous");
        assert_eq!(
            SPEC.parse(&args("--sync=1")).unwrap_err(),
            "option '--sync' doesn't allow an argument"
        );
        assert_eq!(
            SPEC.parse(&args("--root")).unwrap_err(),
            "option '--root' requires an argument"
        );
    }

    #[test]
    fn double_dash_and_single_dash() {
        let p = SPEC.parse(&args("-S - -- -y")).unwrap();
        assert_eq!(keys(&p), [Key::Short('S')]);
        assert_eq!(p.targets, ["-", "-y"]);
    }
}
