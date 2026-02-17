# Filter Expression Grammar

## Overview
Filter expressions allow boolean queries on document fields using comparison operators and logical connectives.

## EBNF Grammar

```ebnf
filter           = or_expression

or_expression    = and_expression ( "OR" and_expression )*

and_expression   = atom_filter ( "AND" atom_filter )*

atom_filter      = "(" filter ")"
                 | "NOT" atom_filter
                 | comparison

comparison       = identifier operator value

identifier       = ( ALPHA | "_" ) ( ALPHA | DIGIT | "_" )*

operator         = "=" | "!=" | ">" | ">=" | "<" | "<="

value            = string_literal | integer | float

string_literal   = "'" ( [^'] )* "'"

integer          = "-"? DIGIT+

float            = "-"? DIGIT+ "." DIGIT+

ALPHA            = [a-zA-Z]
DIGIT            = [0-9]
```

## Operator Precedence
1. `NOT` (highest - unary prefix)
2. `AND` (binary infix)
3. `OR` (lowest - binary infix)

## Keyword vs Field Name Resolution

Keywords (`NOT`, `AND`, `OR`) require word boundaries. Without boundaries, they parse as identifiers:

- `NOT category = 'value'` → keyword NOT + field "category"
- `NOTcategory = 'value'` → field "NOTcategory"
- `ANDroid = 'value'` → field "ANDroid"
- `ORder = 'value'` → field "ORder"

To use keywords as literal field names, quote them:
- `"NOT" = 'value'` → field "NOT"
- `"AND" = 'value'` → field "AND"

Word boundary = whitespace, punctuation, or end of input.

## Examples

### Basic Comparisons
```
category = 'electronics'
category != 'clearance'
price > 100
price >= 100
stock < 10
stock <= 50
```

### Operator Semantics
- `=` : equals (case-insensitive for text)
- `!=` : not equals
- `>` : greater than (exclusive)
- `>=` : greater than or equal (inclusive)
- `<` : less than (exclusive)
- `<=` : less than or equal (inclusive)

For combined ranges, use AND:
```
price >= 100 AND price <= 500    # 100 to 500 inclusive
price > 100 AND price < 500      # 101 to 499 (exclusive bounds)
```

### Logical
```
price > 100 AND category = 'electronics'
category = 'books' OR category = 'movies'
price > 50 AND (category = 'electronics' OR category = 'computers')
```

### Negation
```
NOT category = 'sale'
NOT (price > 1000 AND stock < 5)
price > 100 AND NOT category = 'clearance'
```

### Edge Cases
```
NOTification = 'true'          # field name with keyword prefix
NOT notification = 'false'     # keyword NOT + field "notification"
"OR" = 'pending'               # quoted keyword as field name
```

## Case Insensitivity
Keywords are case-insensitive:
- `NOT`, `not`, `Not` all equivalent
- `AND`, `and`, `And` all equivalent
- `OR`, `or`, `Or` all equivalent

Field names and string values are case-sensitive.

## Implementation Notes
Parser uses nom combinator library with recursive descent. Backtracking allows keyword-prefixed identifiers to parse as field names when keyword match fails at word boundary.