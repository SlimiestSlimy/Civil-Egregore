# protocol: reference

## flags.rs

**`CommandLineToken`**: `OpeningBracket`, `ClosingBracket`, `Flag`,
`Hex`, `Integer`, `Float`, `Text`. **`command_line_tokens(words)`**: each
word a token, brackets split off; **`token_of_word(word)`**.
**`FlagKind`**: `General`, `Terminal`. **`Flag`** `{flag_name, flag_help,
flag_kind, parse_flag}`. **`FlagError`**: `HelpShown`, `WrongLine`.
**`FlagParser`** `{tokens, next_token, flags, command_met}`:
**`new(words, flags)`**; **`parse(clean_parameters)`**, the whole line;
**`parse_flags_within(clean_parameters)`**, a flag's own flags until its
`]` (**`parse_flags`**); **`bracketed(read_within)`**;
**`hex_parameter`**, **`integer_parameter`**, **`float_parameter`**,
**`text_parameter`** (**`next_parameter(wanted)`**); **`expect_token`**,
**`wrong_line`**, **`help_text`**.

## server_parameters.rs

**`ServerParameters`** `{shell_words}`: **`cleaned(flags)`**.
**`CleanServerParameters`** `{parameter_values}`: **`put(parameter_name,
parameter_value)`**, **`hex`**, **`integer`**, **`float`**, **`text`**,
**`nested`**. **`ParameterValue`**: `Hex`, `Integer`, `Float`, `Text`,
`Nested`.
