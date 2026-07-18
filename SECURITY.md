
<!--- This file should be copied to the root of github.com/image-rs/.github to be applied org wide --->

# Security Policy

 The purpose of this policy is to protect users of software built with _image-rs_ crates from harm
 without placing an undue burden on the volunteers maintaining the project.

## Reporting a vulnerability

If you believe that you have discovered a security vulnerability in one of the _image-rs_ crates
(and it doesn't fall under an out-of-scope category listed below) then please report it using
GitHub's [private vulnerability
reporting](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately).

Your report should be concise and human written. Otherwise, we may ask you to rewrite it before
engaging further. We only need enough details to confirm that the vulnerability exists. Please do
not fill out the "Affected products", "Severity", or "Weaknesses" sections (we cannot edit the
template). Those can be addressed later.

Once the vulnerability is confirmed, we'll work with you to remediate it, and then disclose.

## Out-of-scope

The following are examples of defects that are not considered security vulnerabilities and should be
reported as normal bugs:

**Crashes or panics (e.g. triggered from crafted input):** In Rust, panicking is considered safe.
Our libraries extensively use assertions to detect implementation bugs and while we strive not to
panic even on crafted inputs, it isn't guaranteed.

**Contrived soundness bugs:** If there isn't likely to be downstream code that triggers
undefined-behavior due to the bug, then it doesn't qualify as a vulnerability.

**Out-of-memory or other denial of service:** Resource limits are on a best-effort basis.

**Vulnerabilities in tests, benchmarks, or examples:** Downsteam users do not run these directly so
issues like path traversal, soundness bugs, and such are not a concern. The exception here is if
merely running the code could compromise a developer's machine.

**Vulnerabilities in GitHub Actions code:** We don't expose any secrets to our CI and don't use use
[trusted publishing](https://crates.io/docs/trusted-publishing), so the worst that could happen here
is wasting our (free) quota or getting inaccurate test results.
