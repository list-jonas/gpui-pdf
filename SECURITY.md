# Security

PDF files are untrusted input. Parsing, text extraction, and rendering must stay off the UI thread and inside the document worker. Do not execute PDF JavaScript, load external resources automatically, log passwords, or write user PDFs outside the future persistence crate.

Report security issues privately to the repository owner. Do not include confidential PDFs, passwords, private keys, or unredacted documents in an issue.

