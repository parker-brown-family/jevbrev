# Security

jevbrev reads text and JSON and prints JSON. It makes no network requests, reads no environment variables, and never handles an API key. The command reads only the files named on its command line.

Things worth reporting:

- input that makes jevbrev panic, hang, or use unbounded memory;
- a read result that reports a value that is not in the request, a `one:` list, or the caller's `known` values;
- a `.brev` file that passes `check` but makes `http_call` produce a path or method other than the one written in the file.

Report privately through [GitHub's private vulnerability reporting](https://github.com/parker-brown-family/jevbrev/security/advisories/new) for this repository. Please do not open a public issue for these. We aim to reply within a week.
