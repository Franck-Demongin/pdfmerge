use std::fmt;

use crate::error::SpecError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageNumber {
    Absolute(u32),
    Last,
}

impl PageNumber {
    fn resolve(self, page_count: u32) -> u32 {
        match self {
            Self::Absolute(value) => value,
            Self::Last => page_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Page(PageNumber),
    Range(PageNumber, PageNumber),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Item {
    kind: ItemKind,
    position: usize,
}

/// Sélection de pages pour un document.
///
/// `PageSpec` est une valeur immuable qui peut être parsée depuis une chaîne
/// textuelle puis résolue pour un nombre de pages donné. Sans élément, la
/// sélection représente toutes les pages du document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PageSpec {
    items: Vec<Item>,
}

impl PageSpec {
    /// Crée une sélection contenant toutes les pages du document.
    pub fn all() -> Self {
        Self::default()
    }

    pub fn parse(input: &str) -> Result<Self, SpecError> {
        let chars: Vec<char> = input.chars().collect();
        let (mut start, mut end) = trimmed_bounds(&chars);

        if start >= end {
            return Ok(Self::all());
        }

        // Crochets englobants tolérés : `[...]`.
        if chars[start] == '[' {
            if chars[end - 1] != ']' {
                return Err(SpecError::new(start + 1, "unmatched opening bracket"));
            }
            if start + 1 >= end - 1 {
                return Ok(Self::all());
            }
            start += 1;
            end -= 1;
        }

        let mut items = Vec::new();
        let mut i = start;
        skip_ws(&chars, &mut i, end);

        if i >= end {
            return Ok(Self::all());
        }

        while i < end {
            let item = parse_item(&chars, i, end)?;
            i = item.1;
            items.push(item.0);

            skip_ws(&chars, &mut i, end);

            if i < end {
                if chars[i] != ',' {
                    return Err(SpecError::new(
                        i + 1,
                        format!("expected ',' after selection item, found '{}'", chars[i]),
                    ));
                }
                i += 1;
                skip_ws(&chars, &mut i, end);
                if i >= end {
                    return Err(SpecError::new(i + 1, "expected page number after ','"));
                }
            }
        }

        Ok(Self { items })
    }

    /// Résout la sélection pour un document de `page_count` pages.
    pub fn resolve(&self, page_count: u32) -> Result<Vec<u32>, SpecError> {
        if self.items.is_empty() {
            return Ok((1..=page_count).collect());
        }

        let mut pages = Vec::new();
        for item in &self.items {
            match item.kind {
                ItemKind::Page(number) => {
                    let page = number.resolve(page_count);
                    ensure_page(page, page_count, item.position)?;
                    pages.push(page);
                }
                ItemKind::Range(from, to) => {
                    let from_page = from.resolve(page_count);
                    let to_page = to.resolve(page_count);
                    ensure_page(from_page, page_count, item.position)?;
                    ensure_page(to_page, page_count, item.position)?;
                    if from_page <= to_page {
                        pages.extend(from_page..=to_page);
                    } else {
                        for page in (to_page..=from_page).rev() {
                            pages.push(page);
                        }
                    }
                }
            }
        }

        Ok(pages)
    }

    pub fn is_all(&self) -> bool {
        self.items.is_empty()
    }
}

fn parse_item(chars: &[char], i: usize, end: usize) -> Result<(Item, usize), SpecError> {
    let number_start = i;
    let (number, i) = parse_page_number(chars, i, end)?;
    let position = number_start + 1;

    let mut cursor = i;
    skip_ws(chars, &mut cursor, end);

    if cursor < end && chars[cursor] == '-' {
        cursor += 1;
        skip_ws(chars, &mut cursor, end);
        if cursor >= end {
            return Err(SpecError::new(cursor + 1, "missing page number after '-'"));
        }
        let (second, result) = parse_page_number(chars, cursor, end)?;
        let item = Item {
            kind: ItemKind::Range(number, second),
            position,
        };
        return Ok((item, result));
    }

    let item = Item {
        kind: ItemKind::Page(number),
        position,
    };
    Ok((item, cursor))
}

fn parse_page_number(
    chars: &[char],
    mut i: usize,
    end: usize,
) -> Result<(PageNumber, usize), SpecError> {
    if i >= end {
        return Err(SpecError::new(i + 1, "expected a page number"));
    }

    let position = i + 1;

    if chars[i] == '$' {
        return Ok((PageNumber::Last, i + 1));
    }

    if chars[i] == '0' {
        return Err(SpecError::new(
            position,
            "page numbers must not have leading zeros",
        ));
    }
    if chars[i] < '1' || chars[i] > '9' {
        return Err(SpecError::new(
            position,
            format!("expected a positive integer or '$', found '{}'", chars[i]),
        ));
    }

    let mut digits = String::new();
    digits.push(chars[i]);
    i += 1;
    while i < end {
        if chars[i].is_ascii_digit() {
            digits.push(chars[i]);
            i += 1;
        } else {
            break;
        }
    }

    let value = digits
        .parse::<u64>()
        .map_err(|_| SpecError::new(position, "unable to parse page number"))?;
    if value == 0 || value > u32::MAX as u64 {
        return Err(SpecError::new(
            position,
            "page number is out of the supported range",
        ));
    }

    Ok((PageNumber::Absolute(value as u32), i))
}

fn ensure_page(page: u32, page_count: u32, position: usize) -> Result<(), SpecError> {
    if page > page_count {
        return Err(SpecError::new(
            position,
            format!(
                "page {} does not exist (document has {} pages)",
                page, page_count
            ),
        ));
    }
    Ok(())
}

fn skip_ws(chars: &[char], i: &mut usize, end: usize) {
    while *i < end && chars[*i].is_whitespace() {
        *i += 1;
    }
}

fn trimmed_bounds(chars: &[char]) -> (usize, usize) {
    let mut start = 0;
    let mut end = chars.len();
    while start < end && chars[start].is_whitespace() {
        start += 1;
    }
    while end > start && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    (start, end)
}

impl fmt::Display for PageSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.items.is_empty() {
            return write!(f, "<all pages>");
        }
        write!(f, "{} interactions", self.items.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn resolved(expression: &str, page_count: u32) -> Vec<u32> {
        PageSpec::parse(expression)
            .expect("valid page specification")
            .resolve(page_count)
            .expect("valid resolution")
    }

    #[test]
    fn empty_selection_means_all_pages() {
        assert!(PageSpec::parse("").expect("empty text").is_all());
        assert!(PageSpec::parse("   ").expect("spaces").is_all());
        assert!(PageSpec::parse("[ ]").expect("brackets").is_all());
        assert_eq!(resolved("   ", 4), vec![1, 2, 3, 4]);
    }

    #[test]
    fn examples_from_cahier_des_charges() {
        assert_eq!(
            resolved("3-6,1,4-$", 10),
            vec![3, 4, 5, 6, 1, 4, 5, 6, 7, 8, 9, 10]
        );
        assert_eq!(resolved("$", 10), vec![10]);
        assert_eq!(resolved("$-1", 10), (1..=10).rev().collect::<Vec<_>>());
        assert_eq!(resolved("2,2,2", 10), vec![2, 2, 2]);
        assert_eq!(resolved("1-3,8-$", 10), vec![1, 2, 3, 8, 9, 10]);
        assert_eq!(resolved("6-3", 10), vec![6, 5, 4, 3]);
    }

    #[test]
    fn brackets_and_whitespace_are_tolerated() {
        assert_eq!(
            resolved("  [ 3 - 6 , 1 , 4 - $ ]  ", 10),
            vec![3, 4, 5, 6, 1, 4, 5, 6, 7, 8, 9, 10]
        );
    }

    #[test]
    fn out_of_range_pages_report_their_position() {
        let error = PageSpec::parse("12").unwrap().resolve(10).unwrap_err();
        assert_eq!(error.position, 1);
        assert!(error.message.contains("page 12"));
        assert!(error.message.contains("10 pages"));
    }

    #[test]
    fn malformed_inputs_are_rejected() {
        assert!(PageSpec::parse("3-").unwrap_err().position == 3);
        assert!(PageSpec::parse("-3").unwrap_err().position == 1);
        assert!(PageSpec::parse("1,,2").unwrap_err().position > 0);
        assert!(PageSpec::parse("0").unwrap_err().position == 1);
        assert!(PageSpec::parse("a-b").unwrap_err().position == 1);
        assert!(PageSpec::parse("1..2").unwrap_err().position > 0);
        assert!(PageSpec::parse("[3-6").unwrap_err().position == 1);
        assert!(PageSpec::parse("3-6]").unwrap_err().position == 4);
        assert!(
            PageSpec::parse("999999999999999999999")
                .unwrap_err()
                .position
                == 1
        );
    }

    #[test]
    fn extremely_large_page_number_is_rejected() {
        let error = PageSpec::parse("999999999999999999999").unwrap_err();
        assert!(!error.message.is_empty());
    }

    #[test]
    fn ascending_range_covers_the_document() {
        for n in 1u32..=25 {
            let spec = PageSpec::parse(&format!("1-{}", n)).unwrap();
            let pages = spec.resolve(n).unwrap();
            assert_eq!(pages.len(), n as usize);
            assert!(pages.iter().all(|page| (1..=n).contains(page)));
        }
    }

    #[test]
    fn reverse_range_contains_only_document_pages() {
        for n in 1u32..=25 {
            let pages = resolved("$-1", n);
            assert_eq!(pages.len(), n as usize);
            assert!(pages.iter().all(|page| (1..=n).contains(page)));
            assert_eq!(pages, (1..=n).rev().collect::<Vec<_>>());
        }
    }

    #[test]
    fn single_and_duplicate_pages_are_preserved() {
        assert_eq!(resolved("2,2,2", 10), vec![2, 2, 2]);
    }

    proptest! {
        #[test]
        fn out_of_bounds_page_is_rejected(n in 1u32..=20u32) {
            let expression = format!("{}", n + 1);
            let spec = PageSpec::parse(&expression).unwrap();
            prop_assert!(spec.resolve(n).is_err());
        }

        #[test]
        fn reverse_document_is_valid_for_any_size(n in 1u32..=30u32) {
            let pages = PageSpec::parse("$-1").unwrap().resolve(n).unwrap();
            prop_assert!(pages.iter().all(|page| (1..=n).contains(page)));
        }

        #[test]
        fn full_document_range_contains_every_page(n in 1u32..=30u32) {
            let pages = PageSpec::parse(&format!("1-{}", n)).unwrap().resolve(n).unwrap();
            prop_assert!(pages.len() == n as usize);
        }
    }
}
