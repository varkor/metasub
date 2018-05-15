# metasub
`metasub` is a proof-assistant generator for first-order abstract syntax.

## Examples
```bash
# Compile the λ-calculus signature.
> cargo run "src/examples/lambda-calculus.sig"
    Generated a new file at: metasub/out/lambda-calculus-term-verifier.rs
    Compiled the program at: metasub/out/lambda-calculus-term-verifier
# Test the example terms using the generated term verifier.
> out/lambda-calculus-term-verifier src/examples/lambda-calculus.terms
    Term was syntactically correct.
    Generated a construction of the inductive type at: lambda-calculus-inductive-type.v
# Test terms interactively using the generated term verifier.
> out/lambda-calculus-term-verifier -i
    Please enter a term to type check:
> abs (x -> x)
    Term was syntactically correct.
    Please enter a term to type check:
    Generated a construction of the inductive type at: lambda-calculus-inductive-type.v
```
