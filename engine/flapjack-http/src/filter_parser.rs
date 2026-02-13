//! Filter expression parser using nom combinators.
//!
//! Parses boolean filter expressions with syntax:
//! - Comparisons: `field = 'value'`, `price > 100`, `stock <= 50`
//! - Logical: `AND`, `OR`, `NOT`
//! - Grouping: `(price > 50 AND stock > 0) OR featured = 'true'`
//!
//! Keywords require word boundaries. Without boundaries, they parse as identifiers:
//! - `NOT category` → keyword + field
//! - `NOTcategory` → field name
//!
//! See filter_parser_grammar.md for complete grammar.

use flapjack::types::{FieldValue, Filter};
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_while1},
    character::complete::{char, multispace0, multispace1},
    combinator::cut,
    error::context,
    sequence::{delimited, preceded, tuple},
    IResult,
};

/// Parse a filter expression string into a Filter AST.
///
/// # Examples
/// ```
/// use flapjack::http::filter_parser::parse_filter;
///
/// let filter = parse_filter("price > 100 AND category:electronics").unwrap();
/// ```
///
/// # Errors
/// Returns error string if input is malformed or contains unexpected tokens.
pub fn parse_filter(input: &str) -> Result<Filter, String> {
    match filter(input.trim()) {
        Ok(("", f)) => Ok(f),
        Ok((remaining, _)) => Err(format!("Unexpected input after filter: '{}'", remaining)),
        Err(e) => Err(format!("Parse error: {}", e)),
    }
}

fn filter(input: &str) -> IResult<&str, Filter> {
    or_filter(input)
}

fn or_filter(input: &str) -> IResult<&str, Filter> {
    let (input, first) = and_filter(input)?;
    let (input, rest) = nom::multi::many0(preceded(
        delimited(multispace0, keyword("OR"), multispace0),
        cut(and_filter),
    ))(input)?;

    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut filters = vec![first];
        filters.extend(rest);
        Ok((input, Filter::Or(filters)))
    }
}

fn and_filter(input: &str) -> IResult<&str, Filter> {
    let (input, first) = atom_filter(input)?;
    let (input, rest) = nom::multi::many0(preceded(
        delimited(multispace0, keyword("AND"), multispace0),
        cut(atom_filter),
    ))(input)?;

    if rest.is_empty() {
        Ok((input, first))
    } else {
        let mut filters = vec![first];
        filters.extend(rest);
        Ok((input, Filter::And(filters)))
    }
}

fn keyword<'a>(kw: &'static str) -> impl Fn(&'a str) -> IResult<&'a str, &'a str> {
    move |input: &'a str| {
        let (remaining, matched) = tag_no_case(kw)(input)?;

        if remaining
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            return Err(nom::Err::Error(nom::error::Error::new(
                input,
                nom::error::ErrorKind::Tag,
            )));
        }

        Ok((remaining, matched))
    }
}

fn atom_filter(input: &str) -> IResult<&str, Filter> {
    alt((
        delimited(
            char('('),
            delimited(multispace0, filter, multispace0),
            char(')'),
        ),
        not_filter,
        numeric_comparison,
        comparison,
    ))(input)
}

fn not_filter(input: &str) -> IResult<&str, Filter> {
    let (input, _) = preceded(multispace0, keyword("NOT"))(input)?;

    let (input, inner) = cut(preceded(
        multispace1,
        alt((
            delimited(
                char('('),
                delimited(multispace0, filter, multispace0),
                char(')'),
            ),
            not_filter,
            comparison,
        )),
    ))(input)?;
    Ok((input, Filter::Not(Box::new(inner))))
}

fn comparison(input: &str) -> IResult<&str, Filter> {
    let (input, field) = context(
        "field name",
        delimited(multispace0, identifier, multispace0),
    )(input)?;

    let (input, _) = context(
        "colon or comparison operator",
        delimited(multispace0, char(':'), multispace0),
    )(input)?;

    // Try range first (lookahead for TO)
    if let Ok((remaining, (min, _, max))) = tuple((
        number_literal,
        delimited(multispace1, tag_no_case("TO"), multispace1),
        number_literal,
    ))(input)
    {
        return Ok((
            remaining,
            Filter::Range {
                field: field.to_string(),
                min,
                max,
            },
        ));
    }

    // Try facet value (string)
    if let Ok((remaining, text)) = facet_value(input) {
        return Ok((
            remaining,
            Filter::Equals {
                field: field.to_string(),
                value: FieldValue::Text(text.to_string()),
            },
        ));
    }

    Err(nom::Err::Error(nom::error::Error::new(
        input,
        nom::error::ErrorKind::Alt,
    )))
}

fn numeric_comparison(input: &str) -> IResult<&str, Filter> {
    let (input, field) = context(
        "field name",
        delimited(multispace0, identifier, multispace0),
    )(input)?;
    let (input, op) = context(
        "comparison operator",
        delimited(multispace0, operator, multispace0),
    )(input)?;
    let (input, value) = context(
        "numeric value",
        delimited(multispace0, number_value, multispace0),
    )(input)?;

    let field = field.to_string();
    let filter = match op {
        "=" => Filter::Equals { field, value },
        "!=" => Filter::NotEquals { field, value },
        ">" => Filter::GreaterThan { field, value },
        ">=" => Filter::GreaterThanOrEqual { field, value },
        "<" => Filter::LessThan { field, value },
        "<=" => Filter::LessThanOrEqual { field, value },
        _ => unreachable!("operator parser only returns valid operators"),
    };

    Ok((input, filter))
}

fn operator(input: &str) -> IResult<&str, &str> {
    alt((
        tag(">="),
        tag("<="),
        tag("!="),
        tag("="),
        tag(">"),
        tag("<"),
    ))(input)
}

fn identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)
}

fn facet_value(input: &str) -> IResult<&str, &str> {
    alt((quoted_string, identifier))(input)
}

fn number_literal(input: &str) -> IResult<&str, f64> {
    let (input, num_str) = nom::number::complete::recognize_float(input)?;
    let val = num_str.parse::<f64>().map_err(|_| {
        nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Float))
    })?;
    Ok((input, val))
}

fn number_value(input: &str) -> IResult<&str, FieldValue> {
    let (input, num_str) = nom::number::complete::recognize_float(input)?;
    if num_str.contains('.') || num_str.contains('e') || num_str.contains('E') {
        let val = num_str.parse::<f64>().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Float))
        })?;
        Ok((input, FieldValue::Float(val)))
    } else {
        let val = num_str.parse::<i64>().map_err(|_| {
            nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Digit))
        })?;
        Ok((input, FieldValue::Integer(val)))
    }
}

fn quoted_string(input: &str) -> IResult<&str, &str> {
    delimited(char('"'), take_while1(|c| c != '"'), char('"'))(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_facet_filter_simple() {
        let result = parse_filter("category:Electronics");
        assert!(result.is_ok());
        match result.unwrap() {
            Filter::Equals { field, value } => {
                assert_eq!(field, "category");
                assert_eq!(value, FieldValue::Text("Electronics".to_string()));
            }
            _ => panic!("Expected Equals filter"),
        }
    }

    #[test]
    fn test_facet_filter_quoted() {
        let result = parse_filter("author:\"Stephen King\"");
        assert!(result.is_ok());
        match result.unwrap() {
            Filter::Equals { field, value } => {
                assert_eq!(field, "author");
                assert_eq!(value, FieldValue::Text("Stephen King".to_string()));
            }
            _ => panic!("Expected Equals filter"),
        }
    }

    #[test]
    fn test_numeric_comparison() {
        let result = parse_filter("price > 100");
        assert!(result.is_ok());
        match result.unwrap() {
            Filter::GreaterThan { field, value } => {
                assert_eq!(field, "price");
                assert_eq!(value, FieldValue::Integer(100));
            }
            _ => panic!("Expected GreaterThan filter"),
        }
    }

    #[test]
    fn test_numeric_range() {
        let result = parse_filter("price:10.99 TO 100");
        assert!(result.is_ok());
        match result.unwrap() {
            Filter::Range { field, min, max } => {
                assert_eq!(field, "price");
                assert_eq!(min, 10.99);
                assert_eq!(max, 100.0);
            }
            _ => panic!("Expected Range filter"),
        }
    }

    #[test]
    fn test_and_filter() {
        let result = parse_filter("price > 100 AND category:Electronics");
        assert!(result.is_ok());
    }

    #[test]
    fn test_or_filter() {
        let result = parse_filter("category:Electronics OR category:Books");
        assert!(result.is_ok());
    }

    #[test]
    fn test_nested() {
        let result = parse_filter("(price > 100 AND price < 500) OR category:sale");
        assert!(result.is_ok());
    }

    #[test]
    fn test_not_simple() {
        let result = parse_filter("NOT category:Electronics");
        assert!(result.is_ok());
        match result.unwrap() {
            Filter::Not(inner) => match *inner {
                Filter::Equals { field, .. } => assert_eq!(field, "category"),
                _ => panic!("Expected Equals inside Not"),
            },
            _ => panic!("Expected Not filter"),
        }
    }

    #[test]
    fn test_complex_algolia_style() {
        let result = parse_filter("(author:\"Stephen King\" OR genre:Horror) AND price < 20");
        assert!(result.is_ok());
    }
}
