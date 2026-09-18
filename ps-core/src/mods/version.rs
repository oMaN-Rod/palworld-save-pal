use std::cmp::Ordering;

enum Piece<'a> {
    Num(u64),
    Text(&'a str),
}

fn pieces(v: &str) -> Vec<Piece<'_>> {
    let v = v.trim();
    let v = v
        .strip_prefix(['v', 'V'])
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
        .unwrap_or(v);
    v.split(|c: char| matches!(c, '.' | '-' | '_') || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(|p| match p.parse::<u64>() {
            Ok(n) => Piece::Num(n),
            Err(_) => Piece::Text(p),
        })
        .collect()
}

pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (pa, pb) = (pieces(a), pieces(b));
    for i in 0..pa.len().max(pb.len()) {
        let ord = match (pa.get(i), pb.get(i)) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (Some(Piece::Num(x)), Some(Piece::Num(y))) => x.cmp(y),
            (Some(Piece::Num(_)), Some(Piece::Text(_))) => Ordering::Greater,
            (Some(Piece::Text(_)), Some(Piece::Num(_))) => Ordering::Less,
            (Some(Piece::Text(x)), Some(Piece::Text(y))) => {
                x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase())
            }
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

pub fn is_newer(candidate: &str, installed: &str) -> bool {
    compare_versions(candidate, installed) == Ordering::Greater
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn numeric_pieces_compare_numerically() {
        assert_eq!(compare_versions("1.10.0", "1.9.3"), Ordering::Greater);
        assert_eq!(compare_versions("v2.0", "2.0"), Ordering::Equal);
        assert_eq!(compare_versions("1.0", "1.0.1"), Ordering::Less);
    }

    #[test]
    fn mixed_and_odd_strings_degrade_gracefully() {
        assert_eq!(compare_versions("1.2-beta", "1.2-alpha"), Ordering::Greater);
        assert_eq!(
            compare_versions("Palworld_ForPS066", "Palworld_ForPS066"),
            Ordering::Equal
        );
        assert_eq!(
            compare_versions("Palworld_ForPS067", "Palworld_ForPS066"),
            Ordering::Greater
        );
        assert_eq!(compare_versions("", "1"), Ordering::Less);
        assert_eq!(
            compare_versions("unversioned", "unversioned"),
            Ordering::Equal
        );
    }

    #[test]
    fn is_newer_is_strict() {
        assert!(is_newer("1.1", "1.0"));
        assert!(!is_newer("1.0", "1.0"));
        assert!(!is_newer("0.9", "1.0"));
    }
}
