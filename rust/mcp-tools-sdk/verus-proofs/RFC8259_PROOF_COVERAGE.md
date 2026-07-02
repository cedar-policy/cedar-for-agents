RFC 8259 — Proof Coverage Analysis
====================================

This document follows the structure of RFC 8259 (The JavaScript Object Notation
(JSON) Data Interchange Format), quoting each normative requirement and noting
where it is proven, partially proven, or not proven in the verus-proofs codebase.

Legend:
  ✅ PROVEN     — the requirement is formally verified
  ⚠️  PARTIAL   — some aspects are proven, others are not
  ❌ GAP        — not currently proven
  N/A          — not applicable (generator requirement, or informational text)

================================================================================
§2  JSON Grammar
================================================================================

> A JSON text is a sequence of tokens.  The set of tokens includes six
> structural characters, strings, numbers, and three literal names.

✅ PROVEN
  tokenizer.rs: `TokenKind` enum has exactly these kinds:
  - Six structural: ArrayStart, ArrayEnd, ObjectStart, ObjectEnd, Comma, Colon
  - Strings: String
  - Numbers: Number
  - Three literal names: Null, True, False

> A JSON text is a serialized value.

✅ PROVEN
  json_spec.rs: `spec_parse_json` requires parsing exactly one value from the
  complete token stream (returns None if tokens remain after the value).

>    JSON-text = ws value ws

✅ PROVEN
  - json_spec.rs `spec_parse_json`: parses a single value consuming all tokens.
  - tokenizer.rs `tokenize_all` postcondition: gaps between tokens, bytes
    before the first token, and bytes after the last token are all whitespace.
  - Combined: the input consists of whitespace, then one value's tokens, then
    whitespace — nothing else.

> These are the six structural characters:
>
>    begin-array     = ws %x5B ws  ; [ left square bracket
>    begin-object    = ws %x7B ws  ; { left curly bracket
>    end-array       = ws %x5D ws  ; ] right square bracket
>    end-object      = ws %x7D ws  ; } right curly bracket
>    name-separator  = ws %x3A ws  ; : colon
>    value-separator = ws %x2C ws  ; , comma

✅ PROVEN
  tokenizer.rs `token_content_valid`:
  - ArrayStart  => `token.end - token.start == 1 && input[token.start] == LBRACKET()` (0x5B)
  - ArrayEnd    => `token.end - token.start == 1 && input[token.start] == RBRACKET()` (0x5D)
  - ObjectStart => `token.end - token.start == 1 && input[token.start] == LBRACE()` (0x7B)
  - ObjectEnd   => `token.end - token.start == 1 && input[token.start] == RBRACE()` (0x7D)
  - Comma       => `token.end - token.start == 1 && input[token.start] == COMMA()` (0x2C)
  - Colon       => `token.end - token.start == 1 && input[token.start] == COLON()` (0x3A)

  The `ws` surrounding each structural character is proven by `get_token`'s
  postcondition: bytes between `pos` and `token.start` are all whitespace.
  Combined with `tokenize_all`'s gap invariant, inter-token gaps are whitespace.

> Insignificant whitespace is allowed before or after any of the six
> structural characters.
>
>    ws = *(
>            %x20 /              ; Space
>            %x09 /              ; Horizontal tab
>            %x0A /              ; Line feed or New line
>            %x0D )              ; Carriage return

✅ PROVEN
  byte_specs.rs `spec_is_whitespace`:
  `b == SPACE() || b == TAB() || b == NEWLINE() || b == CR()`
  which is `b == 0x20 || b == 0x09 || b == 0x0A || b == 0x0D` — exactly the
  four bytes in the RFC grammar.

  tokenizer.rs `skip_whitespace`: proven to advance past exactly these bytes
  and stop at the first non-whitespace byte (`lemma_skip_whitespace_result_not_ws`,
  `lemma_skip_whitespace_all_ws`).

================================================================================
§3  Values
================================================================================

> A JSON value MUST be an object, array, number, or string, or one of
> the following three literal names:
>
>    false
>    null
>    true

✅ PROVEN
  json_spec.rs `spec_parse_value`: dispatches on token kind covering all 7
  alternatives (Null, True, False, Number, String, ArrayStart→array,
  ObjectStart→object). Any other token kind in value position → returns None.

> The literal names MUST be lowercase.  No other literal names are
> allowed.

✅ PROVEN
  tokenizer.rs `get_token`: only dispatches to keyword parsing on lowercase
  first bytes (0x74 't', 0x66 'f', 0x6E 'n'). Uppercase variants are not
  recognized and produce `ErrUnexpectedToken`.

  tokenizer.rs `token_content_valid` for keywords asserts exact byte values:
  - True:  `LOWER_T, LOWER_R, LOWER_U, LOWER_E` (0x74, 0x72, 0x75, 0x65)
  - False: `LOWER_F, LOWER_A, LOWER_L, LOWER_S, LOWER_E` (0x66, 0x61, 0x6C, 0x73, 0x65)
  - Null:  `LOWER_N, LOWER_U, LOWER_L, LOWER_L` (0x6E, 0x75, 0x6C, 0x6C)

>    value = false / null / true / object / array / number / string

✅ PROVEN (see above — `spec_parse_value` covers all alternatives)

>    false = %x66.61.6c.73.65   ; false

✅ PROVEN — `token_content_valid` for False asserts these exact 5 bytes.

>    null  = %x6e.75.6c.6c      ; null

✅ PROVEN — `token_content_valid` for Null asserts these exact 4 bytes.

>    true  = %x74.72.75.65      ; true

✅ PROVEN — `token_content_valid` for True asserts these exact 4 bytes.

================================================================================
§4  Objects
================================================================================

> An object structure is represented as a pair of curly brackets
> surrounding zero or more name/value pairs (or members).  A name is a
> string.  A single colon comes after each name, separating the name
> from the value.  A single comma separates a value from a following
> name.

✅ PROVEN
  json_spec.rs `spec_parse_object` / `spec_parse_object_members`:
  - First token must be ObjectEnd (empty object) or String (key).
  - After the key: Colon token required, then a value.
  - After the value: ObjectEnd (done) or Comma (then next member).

  parser.rs `parse_object_body`: proven to match this spec exactly.

> The names within an object SHOULD be unique.

✅ PROVEN (stricter than required — rejects duplicates)
  json_spec.rs `spec_parse_object_members`: calls `spec_key_exists` before
  adding each key. If duplicate found → returns None (parse failure).

  parser.rs `parse_object_body`: performs duplicate detection via `slices_equal`
  on decoded key bytes. Postcondition includes `keys_are_distinct(entries@)`.

  NOTE: RFC says SHOULD (not MUST). This implementation is stricter, which is a
  valid interoperability choice per §4 paragraph 3 of the RFC.

>    object = begin-object [ member *( value-separator member ) ]
>             end-object

✅ PROVEN — `spec_parse_object` implements exactly this grammar:
  empty object (immediate ObjectEnd) or one-or-more members separated by commas.

>    member = string name-separator value

✅ PROVEN — `spec_parse_object_members` requires: String token, then Colon
  token, then recursive `spec_parse_value`.

================================================================================
§5  Arrays
================================================================================

> An array structure is represented as square brackets surrounding zero
> or more values (or elements).  Elements are separated by commas.

✅ PROVEN
  json_spec.rs `spec_parse_array` / `spec_parse_array_elements`:
  - ArrayEnd immediately after ArrayStart → empty array.
  - Otherwise: parse value, then expect ArrayEnd or Comma+next element.

>    array = begin-array [ value *( value-separator value ) ] end-array

✅ PROVEN — `spec_parse_array` implements exactly this grammar.

  Trailing commas (e.g. `[1,]`) are correctly rejected: after Comma, the spec
  recurses to `spec_parse_value` which would see ArrayEnd — a structural token
  in value position → returns None.

> There is no requirement that the values in an array be of the same
> type.

✅ PROVEN (trivially) — `spec_parse_array_elements` calls `spec_parse_value`
  for each element with no type restriction.

================================================================================
§6  Numbers
================================================================================

> The representation of numbers is similar to that used in most
> programming languages.  A number is represented in base 10 using
> decimal digits.  It contains an integer component that may be
> prefixed with an optional minus sign, which may be followed by a
> fraction part and/or an exponent part.  Leading zeros are not
> allowed.

✅ PROVEN — see grammar items below.

>    number = [ minus ] int [ frac ] [ exp ]

✅ PROVEN
  tokenizer.rs `spec_number_end`:
  - Optional leading '-' (`if input[pos] == DASH()`)
  - `spec_int_part_end` (required)
  - `spec_frac_part_end` (optional)
  - `spec_exp_part_end` (optional)

  `consume_number` is proven to return Ok only when `spec_is_valid_json_number`
  holds (which asserts `spec_number_end(input, start) == Some(end)`).

>    decimal-point = %x2E       ; .

✅ PROVEN — `spec_frac_part_end` checks `input[pos] != DOT()` where
  `DOT() = 0x2E`.

>    digit1-9 = %x31-39         ; 1-9

✅ PROVEN — `spec_int_part_end` checks `input[pos] >= 0x31 && input[pos] <= 0x39`.

>    e = %x65 / %x45            ; e E

✅ PROVEN — `spec_exp_part_end` checks
  `input[pos] != LOWER_E() && input[pos] != UPPER_E()` where
  LOWER_E = 0x65, UPPER_E = 0x45.

>    exp = e [ minus / plus ] 1*DIGIT

✅ PROVEN — `spec_exp_part_end`: after 'e'/'E', optional '+'/'-', then requires
  at least one digit (`spec_is_ascii_digit`), consumes remaining via
  `spec_consume_digits`.

>    frac = decimal-point 1*DIGIT

✅ PROVEN — `spec_frac_part_end`: after '.', requires at least one digit,
  consumes remaining via `spec_consume_digits`.

>    int = zero / ( digit1-9 *DIGIT )

✅ PROVEN — `spec_int_part_end`:
  - If byte is '0' (ZERO = 0x30): returns pos+1 unless next byte is also a digit
    (rejects leading zeros).
  - If byte is 0x31-0x39: advances past remaining digits via `spec_consume_digits`.

>    minus = %x2D               ; -

✅ PROVEN — `spec_number_end` checks `input[pos] == DASH()` where DASH = 0x2D.

>    plus = %x2B                ; +

✅ PROVEN — `spec_exp_part_end` checks `input[after_e] == PLUS()` where
  PLUS = 0x2B.

>    zero = %x30                ; 0

✅ PROVEN — `spec_int_part_end` checks `input[pos] == ZERO()` where ZERO = 0x30.

  Leading zeros:
> (implied: int = zero means a single zero; zero followed by digit is invalid)

✅ PROVEN — `spec_int_part_end`: when `input[pos] == ZERO()`, checks
  `pos + 1 < input.len() && spec_is_ascii_digit(input[pos + 1])` → returns None.
  `consume_int_part` exec: same check, returns None on leading zero.

  Number byte completeness:
✅ PROVEN — `token_content_valid` for Number includes
  `spec_all_number_bytes(input, start, end)`: every byte in the token span is
  one of: digit, '-', '+', '.', 'e', 'E'. No other characters can appear.

================================================================================
§7  Strings
================================================================================

> The representation of strings is similar to conventions used in the C
> family of programming languages.  A string begins and ends with
> quotation marks.

✅ PROVEN
  - Opening quote: `token_content_valid` for String asserts
    `input[token.start] == QUOTE()` (0x22).
  - Closing quote: `token_content_valid` for String asserts
    `input[(token.end - 1) as int] == QUOTE()` (0x22).
  - `consume_string` postcondition: `input[(end - 1) as int] == QUOTE()`.

> All Unicode characters may be placed within the
> quotation marks, except for the characters that MUST be escaped:
> quotation mark, reverse solidus, and the control characters (U+0000
> through U+001F).

✅ PROVEN (rejection of unescaped mandatory-escape characters)
  tokenizer.rs `consume_string`:
  - `if b == 0x22` → closing quote (terminates string, not content)
  - `if b == 0x5C` → backslash, enters escape processing
  - `if b < 0x20` → `return StringResult::InvalidEscape { pos: i }` — rejects
    all control characters U+0000 through U+001F.

  This correctly enforces: quotation marks terminate, reverse solidus starts
  escape, control chars are rejected.

> Any character may be escaped.  If the character is in the Basic
> Multilingual Plane (U+0000 through U+FFFF), then it may be
> represented as a six-character sequence: a reverse solidus, followed
> by the lowercase letter u, followed by four hexadecimal digits that
> encode the character's code point.

✅ PROVEN
  escape.rs `spec_decode`: handles `esc == LOWER_U()` with `spec_is_hex_quad`
  (validates 4 hex digits) then `spec_decode_hex4` (computes value ≤ 0xFFFF).
  Non-surrogate BMP code points → `spec_encode_code_point(cp)` → UTF-8 output.

> The hexadecimal letters A through F can be uppercase or lowercase.

✅ PROVEN
  byte_specs.rs `spec_is_hex_digit`:
  `(ZERO() <= b && b <= NINE()) || (LOWER_A() <= b && b <= LOWER_F()) || (UPPER_A() <= b && b <= UPPER_F())`
  Accepts both cases.

> So, for example, a string containing only a single reverse solidus
> character may be represented as "\u005C".

✅ PROVEN (follows from BMP escape decoding — 0x005C is non-surrogate BMP,
  decoded via `spec_encode_code_point(0x5C)` → `seq![0x5C]`).

> Alternatively, there are two-character sequence escape
> representations of some popular characters.  So, for example, a
> string containing only a single reverse solidus character may be
> represented more compactly as "\\".

✅ PROVEN
  escape.rs `spec_simple_escape_byte`: `BACKSLASH() → BACKSLASH()` (0x5C → 0x5C).
  byte_specs.rs `spec_is_simple_escape`: includes BACKSLASH (0x5C).

> To escape an extended character that is not in the Basic Multilingual
> Plane, the character is represented as a 12-character sequence,
> encoding the UTF-16 surrogate pair.  So, for example, a string
> containing only the G clef character (U+1D11E) may be represented as
> "\uD834\uDD1E".

✅ PROVEN
  escape.rs `spec_decode`: when `is_high_surrogate(cp)`:
  - Expects `\u` (backslash + 'u') at offset +6
  - Validates 4 hex digits for low surrogate
  - Checks `is_low_surrogate(low)`
  - Computes: `surrogate_pair_value(hi, lo)` =
    `0x10000 + (hi - 0xD800) * 0x400 + (lo - 0xDC00)`
  - Outputs `spec_encode_code_point(full)` → 4-byte UTF-8

  byte_specs.rs `surrogate_pair_value`: implements the standard formula.
  Lone surrogates (high without low, or lone low) → returns None (rejected).

>    string = quotation-mark *char quotation-mark

✅ PROVEN
  `token_content_valid` for String: opening quote at `token.start`, closing
  quote at `token.end - 1`. Content between is validated by `consume_string`
  (rejects control chars, validates escape sequences).

>    char = unescaped /
>        escape (
>            %x22 /          ; "    quotation mark  U+0022
>            %x5C /          ; \    reverse solidus U+005C
>            %x2F /          ; /    solidus         U+002F
>            %x62 /          ; b    backspace       U+0008
>            %x66 /          ; f    form feed       U+000C
>            %x6E /          ; n    line feed       U+000A
>            %x72 /          ; r    carriage return U+000D
>            %x74 /          ; t    tab             U+0009
>            %x75 4HEXDIG )  ; uXXXX                U+XXXX

✅ PROVEN (escape decoding)
  byte_specs.rs `spec_is_simple_escape`: covers 0x22, 0x5C, 0x2F, 0x62, 0x66,
  0x6E, 0x72, 0x74 — all 8 simple escape characters.

  escape.rs `spec_simple_escape_byte` maps each to its decoded value:
  - 0x22 → 0x22 (quotation mark)
  - 0x5C → 0x5C (reverse solidus)
  - 0x2F → 0x2F (solidus)
  - 0x62 → 0x08 (backspace)
  - 0x66 → 0x0C (form feed)
  - 0x6E → 0x0A (line feed)
  - 0x72 → 0x0D (carriage return)
  - 0x74 → 0x09 (tab)

  Unicode escape (0x75 + 4HEXDIG): handled by `spec_decode` BMP and surrogate
  pair branches with `spec_is_hex_quad` validation.

  Unknown escape characters (not in this list and not 'u') → `spec_decode`
  returns None. `consume_string` in tokenizer also rejects them.

>    escape = %x5C              ; \

✅ PROVEN — `BACKSLASH() = 0x5C` used throughout. `consume_string` enters
  escape processing on byte 0x5C. `spec_decode` checks `input[start] == BACKSLASH()`.

>    quotation-mark = %x22      ; "

✅ PROVEN — `QUOTE() = 0x22` used throughout. Token starts with quote
  (proven in `token_content_valid`).

>    unescaped = %x20-21 / %x23-5B / %x5D-10FFFF

✅ PROVEN
  tokenizer.rs `consume_string` validates unescaped bytes via
  `utf8_validation::validate_utf8_char`, which is proven against vstd's
  `valid_first_scalar`. This ensures:
  - Single bytes 0x20-0x7F (excluding 0x22 quote and 0x5C backslash): accepted
    as valid 1-byte UTF-8 (ASCII). Quote and backslash are handled before
    reaching the UTF-8 validator.
  - Multi-byte sequences (0xC2-0xF4 leading byte + continuation bytes): accepted
    only if they form valid UTF-8 per RFC 3629 — rejects overlong encodings,
    surrogate code points, and code points > U+10FFFF.
  - Bare continuation bytes (0x80-0xBF) and invalid leading bytes (0xC0-0xC1,
    0xF5-0xFF): rejected.
  - Control characters (< 0x20): rejected in the preceding branch.

  This matches the RFC's `unescaped` production which operates on Unicode code
  points %x20-21 / %x23-5B / %x5D-10FFFF — every accepted byte sequence
  corresponds to a valid Unicode scalar value in these ranges.

================================================================================
§8  String and Character Issues
================================================================================

§8.1 Character Encoding

> JSON text exchanged between systems that are not part of a closed
> ecosystem MUST be encoded using UTF-8 [RFC3629].

✅ PROVEN (for string content)
  - The codebase operates on `&[u8]` / `Seq<u8>` — the correct domain for UTF-8.
  - Escape decoding produces valid UTF-8: `encode_code_point` is proven to emit
    well-formed UTF-8 (`lemma_encode_wellformed` in byte_specs.rs).
  - String content validation: `consume_string` calls `validate_utf8_char` for
    all unescaped bytes, which is proven against vstd's `valid_first_scalar`.
    Invalid UTF-8 sequences are rejected at tokenization time.
  - Keywords (true/false/null) and structural tokens are ASCII (trivially UTF-8).
  - Number tokens contain only ASCII digits and `-+.eE` (trivially UTF-8).
  - Whitespace is ASCII (trivially UTF-8).
  - Therefore: if tokenization succeeds, all bytes in the input are valid UTF-8.

> Implementations MUST NOT add a byte order mark (U+FEFF) to the
> beginning of a networked-transmitted JSON text.

N/A — generator requirement, not applicable to a parser.

> In the interests of interoperability, implementations that parse JSON
> texts MAY ignore the presence of a byte order mark rather than
> treating it as an error.

N/A — optional behavior. The current parser does NOT ignore a BOM (it would be
  an unrecognized byte causing a tokenization error). This is conformant since
  the RFC says MAY, not MUST.

§8.2 Unicode Characters

> When all the strings represented in a JSON text are composed entirely
> of Unicode characters [UNICODE] (however escaped), then that JSON
> text is interoperable in the sense that all software implementations
> that parse it will agree on the contents of names and of string
> values in objects and arrays.

N/A — informational/interoperability guidance.

> However, the ABNF in this specification allows member names and
> string values to contain bit sequences that cannot encode Unicode
> characters; for example, "\uDEAD" (a single unpaired UTF-16
> surrogate).

✅ PROVEN (rejected)
  escape.rs `spec_decode`: when `is_low_surrogate(cp)` (which includes 0xDEAD
  since 0xDC00 ≤ 0xDEAD ≤ 0xDFFF) in starting position → returns None.
  Lone high surrogates not followed by a valid `\uLLLL` → also returns None.

  NOTE: The RFC *allows* this but notes behavior is "unpredictable". Our spec
  is stricter (rejects unpaired surrogates in escape sequences), which is a
  valid implementation choice.

§8.3 String Comparison

> Implementations that transform the textual representation into
> sequences of Unicode code units and then perform the comparison
> numerically, code unit by code unit, are interoperable in the sense
> that implementations will agree in all cases on equality or
> inequality of two strings.  For example, implementations that compare
> strings with escaped characters unconverted may incorrectly find that
> "a\\b" and "a\u005Cb" are not equal.

✅ PROVEN
  parser.rs `parse_object_body`: duplicate key detection compares *decoded*
  key bytes via `slices_equal` (proven: `result == (a@ =~= b@)` in dedup.rs).
  Since escape decoding is proven correct, `"a\\b"` and `"a\u005Cb"` both
  decode to the same bytes `[0x61, 0x5C, 0x62]` and are correctly detected
  as equal (duplicate).

================================================================================
§9  Parsers
================================================================================

> A JSON parser transforms a JSON text into another representation.

✅ PROVEN — `parse_json` transforms `&[u8]` into `JsonValue`.

> A JSON parser MUST accept all texts that conform to the JSON grammar.

⚠️ PARTIAL
  - SOUNDNESS proven: if `parse_json` returns Ok, the result matches `spec_parse_json`.
  - COMPLETENESS not proven: we don't prove that every valid JSON input causes
    `parse_json` to return Ok (as opposed to Err).
  - The `ParseResult::Err` postcondition is simply `true` (no guarantees on failure).
  - To prove completeness would require:
    (a) Proving tokenizer completeness (tokenize_all returns Ok for valid input)
    (b) Proving fuel sufficiency (spec_parse_value with sufficient fuel always
        returns Some for well-formed token streams)

> A JSON parser MAY accept non-JSON forms or extensions.

N/A — optional. This parser does not accept extensions.

> An implementation may set limits on the size of texts that it
> accepts.

N/A — informational. No explicit size limit in this implementation (bounded
  only by `usize`).

> An implementation may set limits on the maximum depth of nesting.

⚠️ PARTIAL
  The parser uses `gas` (fuel) parameter which effectively limits nesting depth
  to `tokens.len()`. This is a generous bound but technically a limit. The fuel
  sufficiency proof (showing this bound is always enough for valid input) is not
  yet done.

> An implementation may set limits on the range and precision of
> numbers.

N/A — numbers are stored as raw byte spans (`Number { start, end }`), not
  interpreted as numeric values. No precision loss possible.

> An implementation may set limits on the length and character contents
> of strings.

N/A — no string length limit in this implementation.

================================================================================
§10 Generators
================================================================================

N/A — this codebase is a parser, not a generator.

================================================================================
§12 Security Considerations
================================================================================

> Generally, there are security issues with scripting languages.  JSON
> is a subset of JavaScript but excludes assignment and invocation.

N/A — informational.

> Since JSON's syntax is borrowed from JavaScript, it is possible to
> use that language's "eval()" function to parse most JSON texts

N/A — informational, not relevant to this verified parser.

================================================================================
SUMMARY
================================================================================

Fully proven (✅):
  - §2: JSON-text grammar, structural characters, whitespace definition
  - §3: All 7 value types, keyword byte sequences, lowercase enforcement
  - §4: Object grammar, member structure, duplicate key rejection (SHOULD→MUST)
  - §5: Array grammar, trailing comma rejection, heterogeneous elements
  - §6: Complete number grammar (int, frac, exp, leading zero rejection)
  - §7: String delimiters (opening and closing quotes), all escape sequences
         (simple, BMP \uXXXX, surrogate pairs \uHHHH\uLLLL), control character
         rejection, hex case insensitivity, UTF-8 validation of unescaped content
         (proven against vstd::utf8::valid_first_scalar)
  - §8.1: UTF-8 encoding enforced — invalid UTF-8 in string content rejected
  - §8.2: Unpaired surrogate rejection in escape sequences
  - §8.3: String comparison after escape decoding (duplicate key detection)

Partial (⚠️):

  1. Completeness — accept all valid texts (§9)
     - Soundness (Ok → correct) is proven. Completeness (valid → Ok) is not.
     - Severity: MEDIUM — meaningful property, requires fuel sufficiency proof
       and tokenizer completeness proof.
     - Fix: Prove fuel determinism (sufficient fuel always exists for valid
       token streams). Then prove tokenize_all returns Ok for RFC-valid input.
       Then prove parse_value returns Ok when spec_parse_value returns Some.

  2. Fuel / nesting depth sufficiency (§9)
     - `gas = tokens.len()` is used as fuel. Not proven that this is always
       sufficient for all valid inputs.
     - Severity: MEDIUM — subsumed by gap 1.
     - Fix: Prove `spec_parse_value(input, tokens, 0, tokens.len())` returns
       Some for any well-formed token stream.
