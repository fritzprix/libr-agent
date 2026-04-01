## 2024-05-20 - SQLite `VACUUM INTO` String Interpolation Risk
**Vulnerability:** A file path was interpolated directly into a `VACUUM INTO` SQL string (`format!("VACUUM INTO '{}'", backup_path)`).
**Learning:** SQLite's `VACUUM INTO` requires a string literal and cannot be parameterized with standard placeholder bindings in many drivers (like SeaORM). Direct interpolation of paths containing single quotes (`'`) breaks the query and allows SQL injection.
**Prevention:** Always manually escape single quotes by replacing them with two single quotes (`''`) before interpolating paths into SQL string literals when parameterization is not supported.
