use crate::diag::Diagnostics;
use crate::span::Span;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum MacroDef {
    Object(String),
    Function {
        params: Vec<String>,
        body: String,
        is_variadic: bool,
    },
}

#[derive(Debug, Clone)]
pub struct CondFrame {
    pub parent_active: bool,
    pub branch_taken: bool,
    pub is_active: bool,
}

pub struct Preprocessor {
    include_paths: Vec<PathBuf>,
    macros: HashMap<String, MacroDef>,
}

impl Default for Preprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Preprocessor {
    pub fn new() -> Self {
        let mut p = Self {
            include_paths: Vec::new(),
            macros: HashMap::new(),
        };

        // Predefine standard macros
        p.define_object("__STDC__", "1");
        p.define_object("__STDC_VERSION__", "199901L");
        p.define_object("__rscc__", "1");
        p.define_object("__rcc__", "1");
        p.define_object("__LP64__", "1");
        p.define_object("_LP64", "1");

        #[cfg(target_os = "macos")]
        {
            p.define_object("__APPLE__", "1");
            p.define_object("__MACH__", "1");
        }
        #[cfg(target_os = "linux")]
        {
            p.define_object("__linux__", "1");
            p.define_object("__gnu_linux__", "1");
        }
        #[cfg(target_arch = "aarch64")]
        {
            p.define_object("__aarch64__", "1");
            p.define_object("__arm64__", "1");
            p.define_object("__arm64", "1");
        }
        #[cfg(target_arch = "x86_64")]
        {
            p.define_object("__x86_64__", "1");
        }

        p
    }

    pub fn add_include_path<P: AsRef<Path>>(&mut self, path: P) {
        self.include_paths.push(path.as_ref().to_path_buf());
    }

    pub fn define_object(&mut self, name: &str, value: &str) {
        self.macros
            .insert(name.to_string(), MacroDef::Object(value.to_string()));
    }

    pub fn process(&mut self, source: &str, current_file: &str, diag: &mut Diagnostics) -> String {
        let mut output = String::new();
        let spliced = source.replace("\\\r\n", "").replace("\\\n", "");
        let decommented = strip_comments(&spliced);
        let lines: Vec<&str> = decommented.lines().collect();
        let mut line_idx = 0;
        let mut cond_stack: Vec<CondFrame> = Vec::new();

        while line_idx < lines.len() {
            let line = lines[line_idx];
            let trimmed = line.trim();

            if let Some(stripped) = trimmed.strip_prefix('#') {
                let directive_line = stripped.trim();
                let mut parts = directive_line.split_whitespace();
                let directive = parts.next().unwrap_or("");
                let rest = directive_line[directive.len()..].trim();

                let is_active = cond_stack.last().is_none_or(|f| f.is_active);

                match directive {
                    "ifdef" => {
                        let parent_active = is_active;
                        let name = parts.next().unwrap_or("");
                        let cond = self.macros.contains_key(name);
                        let active = parent_active && cond;
                        cond_stack.push(CondFrame {
                            parent_active,
                            branch_taken: active,
                            is_active: active,
                        });
                    }
                    "ifndef" => {
                        let parent_active = is_active;
                        let name = parts.next().unwrap_or("");
                        let cond = !self.macros.contains_key(name);
                        let active = parent_active && cond;
                        cond_stack.push(CondFrame {
                            parent_active,
                            branch_taken: active,
                            is_active: active,
                        });
                    }
                    "if" => {
                        let parent_active = is_active;
                        let cond = if parent_active {
                            self.eval_if_expr(rest) != 0
                        } else {
                            false
                        };
                        let active = parent_active && cond;
                        cond_stack.push(CondFrame {
                            parent_active,
                            branch_taken: active,
                            is_active: active,
                        });
                    }
                    "elif" => {
                        if let Some(frame) = cond_stack.last_mut() {
                            if !frame.branch_taken && frame.parent_active {
                                let cond = self.eval_if_expr(rest) != 0;
                                if cond {
                                    frame.branch_taken = true;
                                    frame.is_active = true;
                                } else {
                                    frame.is_active = false;
                                }
                            } else {
                                frame.is_active = false;
                            }
                        } else {
                            diag.error(Span::dummy(), "#elif without #if");
                        }
                    }
                    "else" => {
                        if let Some(frame) = cond_stack.last_mut() {
                            if !frame.branch_taken && frame.parent_active {
                                frame.branch_taken = true;
                                frame.is_active = true;
                            } else {
                                frame.is_active = false;
                            }
                        } else {
                            diag.error(Span::dummy(), "#else without #if");
                        }
                    }
                    "endif" => {
                        if cond_stack.pop().is_none() {
                            diag.error(Span::dummy(), "#endif without #if");
                        }
                    }
                    "define" if is_active => {
                        self.parse_define(rest, diag);
                    }
                    "undef" if is_active => {
                        let name = parts.next().unwrap_or("");
                        self.macros.remove(name);
                    }
                    "include" if is_active => {
                        let inc_content = self.handle_include(rest, current_file, diag);
                        output.push_str(&inc_content);
                        output.push('\n');
                    }
                    _ => {
                        // Unknown or inactive directive
                    }
                }
            } else {
                let is_active = cond_stack.last().is_none_or(|f| f.is_active);
                if is_active {
                    let mut combined = line.to_string();
                    let start_line_num = line_idx + 1;
                    while self.has_unclosed_macro_call(&combined) && line_idx + 1 < lines.len() {
                        let next_line = lines[line_idx + 1];
                        let next_trimmed = next_line.trim();
                        if next_trimmed.starts_with('#') {
                            break;
                        }
                        combined.push('\n');
                        combined.push_str(next_line);
                        line_idx += 1;
                    }
                    let expanded = self.expand_line(&combined, current_file, start_line_num);
                    output.push_str(&expanded);
                    output.push('\n');
                }
            }

            line_idx += 1;
        }

        output
    }

    fn parse_define(&mut self, text: &str, _diag: &mut Diagnostics) {
        if text.is_empty() {
            return;
        }

        // Check if function-like macro: `NAME(a, b) body`
        if let Some(paren_pos) = text.find('(') {
            let space_pos = text.find(char::is_whitespace).unwrap_or(text.len());
            if paren_pos < space_pos {
                let name = &text[..paren_pos];
                let after_paren = &text[paren_pos + 1..];
                if let Some(close_paren) = after_paren.find(')') {
                    let param_str = &after_paren[..close_paren];
                    let body = after_paren[close_paren + 1..].trim();

                    let mut params = Vec::new();
                    let mut is_variadic = false;
                    for p in param_str.split(',') {
                        let p = p.trim();
                        if p == "..." {
                            is_variadic = true;
                        } else if !p.is_empty() {
                            params.push(p.to_string());
                        }
                    }

                    self.macros.insert(
                        name.to_string(),
                        MacroDef::Function {
                            params,
                            body: body.to_string(),
                            is_variadic,
                        },
                    );
                    return;
                }
            }
        }

        let mut parts = text.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").trim();
        let value = parts.next().unwrap_or("").trim();

        if !name.is_empty() {
            self.macros
                .insert(name.to_string(), MacroDef::Object(value.to_string()));
        }
    }

    fn eval_if_expr(&self, expr: &str) -> i64 {
        // Step 1: Replace defined(ID) and defined ID
        let text = expr.to_string();
        let mut idx = 0;
        let mut defined_expanded = String::new();
        let bytes = text.as_bytes();
        while idx < bytes.len() {
            if text[idx..].starts_with("defined") {
                let after = idx + 7;
                let before_ok =
                    idx == 0 || (!bytes[idx - 1].is_ascii_alphanumeric() && bytes[idx - 1] != b'_');
                let after_ok = after >= bytes.len()
                    || (!bytes[after].is_ascii_alphanumeric() && bytes[after] != b'_');
                if before_ok && after_ok {
                    // Skip whitespace
                    let mut cur = after;
                    while cur < bytes.len() && bytes[cur].is_ascii_whitespace() {
                        cur += 1;
                    }
                    if cur < bytes.len() && bytes[cur] == b'(' {
                        cur += 1;
                        while cur < bytes.len() && bytes[cur].is_ascii_whitespace() {
                            cur += 1;
                        }
                        let id_start = cur;
                        while cur < bytes.len()
                            && (bytes[cur].is_ascii_alphanumeric() || bytes[cur] == b'_')
                        {
                            cur += 1;
                        }
                        let id = &text[id_start..cur];
                        while cur < bytes.len() && bytes[cur].is_ascii_whitespace() {
                            cur += 1;
                        }
                        if cur < bytes.len() && bytes[cur] == b')' {
                            cur += 1;
                        }
                        let is_def = if self.macros.contains_key(id) {
                            "1"
                        } else {
                            "0"
                        };
                        defined_expanded.push_str(is_def);
                        idx = cur;
                        continue;
                    } else {
                        let id_start = cur;
                        while cur < bytes.len()
                            && (bytes[cur].is_ascii_alphanumeric() || bytes[cur] == b'_')
                        {
                            cur += 1;
                        }
                        let id = &text[id_start..cur];
                        let is_def = if self.macros.contains_key(id) {
                            "1"
                        } else {
                            "0"
                        };
                        defined_expanded.push_str(is_def);
                        idx = cur;
                        continue;
                    }
                }
            }
            defined_expanded.push(bytes[idx] as char);
            idx += 1;
        }

        // Step 2: Expand object macros repeatedly
        let mut expanded = defined_expanded;
        for _ in 0..16 {
            let mut changed = false;
            for (name, def) in &self.macros {
                if let MacroDef::Object(val) = def {
                    let next = replace_identifier(&expanded, name, val);
                    if next != expanded {
                        expanded = next;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }

        // Step 3: Replace remaining identifiers (except true/false) with 0
        let mut final_text = String::new();
        let exp_bytes = expanded.as_bytes();
        let mut cur = 0;
        while cur < exp_bytes.len() {
            let b = exp_bytes[cur];
            if b.is_ascii_alphabetic() || b == b'_' {
                let start = cur;
                while cur < exp_bytes.len()
                    && (exp_bytes[cur].is_ascii_alphanumeric() || exp_bytes[cur] == b'_')
                {
                    cur += 1;
                }
                let id = &expanded[start..cur];
                if id == "true" {
                    final_text.push('1');
                } else {
                    final_text.push('0');
                }
            } else {
                final_text.push(b as char);
                cur += 1;
            }
        }

        // Step 4: Parse and evaluate constant expression
        eval_pp_const_expr(&final_text)
    }

    fn handle_include(&mut self, rest: &str, current_file: &str, diag: &mut Diagnostics) -> String {
        let trimmed = rest.trim();
        let (is_system, include_name) = if let Some(inner) =
            trimmed.strip_prefix('<').and_then(|s| s.strip_suffix('>'))
        {
            (true, inner)
        } else if let Some(inner) = trimmed.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
            (false, inner)
        } else {
            diag.error(
                Span::dummy(),
                format!("Invalid #include syntax: {}", trimmed),
            );
            return String::new();
        };

        // Try relative to current file first
        if !is_system {
            let current_dir = Path::new(current_file)
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let candidate = current_dir.join(include_name);
            if candidate.exists()
                && let Ok(content) = std::fs::read_to_string(&candidate)
            {
                return self.process(&content, candidate.to_str().unwrap_or(current_file), diag);
            }
        }

        // Try include paths
        for dir in &self.include_paths {
            let candidate = dir.join(include_name);
            if candidate.exists()
                && let Ok(content) = std::fs::read_to_string(&candidate)
            {
                return self.process(&content, candidate.to_str().unwrap_or(include_name), diag);
            }
        }

        // Return empty if not found (or standard system header)
        String::new()
    }

    fn has_unclosed_macro_call(&self, text: &str) -> bool {
        let bytes = text.as_bytes();
        let mut i = 0;
        let mut in_macro = false;
        let mut depth = 0;

        while i < bytes.len() {
            let b = bytes[i];
            if b == b'"' || b == b'\'' {
                let quote = b;
                i += 1;
                let mut escape = false;
                while i < bytes.len() {
                    let c = bytes[i];
                    i += 1;
                    if escape {
                        escape = false;
                    } else if c == b'\\' {
                        escape = true;
                    } else if c == quote {
                        break;
                    }
                }
            } else if in_macro {
                if b == b'(' {
                    depth += 1;
                } else if b == b')' {
                    depth -= 1;
                    if depth == 0 {
                        in_macro = false;
                    }
                }
                i += 1;
            } else if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &text[start..i];
                if let Some(MacroDef::Function { .. }) = self.macros.get(ident) {
                    let mut p = i;
                    while p < bytes.len() && (bytes[p] == b' ' || bytes[p] == b'\t') {
                        p += 1;
                    }
                    if p < bytes.len() && bytes[p] == b'(' {
                        in_macro = true;
                        depth = 1;
                        i = p + 1;
                    }
                }
            } else {
                i += 1;
            }
        }

        in_macro && depth > 0
    }

    fn expand_tokens(
        &self,
        text: &str,
        current_file: &str,
        line_num: usize,
        disabled: &HashSet<String>,
    ) -> String {
        let mut out = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            let b = bytes[i];
            if b == b'"' || b == b'\'' {
                let quote = b;
                out.push(quote as char);
                i += 1;
                let mut escape = false;
                while i < bytes.len() {
                    let c = bytes[i];
                    out.push(c as char);
                    i += 1;
                    if escape {
                        escape = false;
                    } else if c == b'\\' {
                        escape = true;
                    } else if c == quote {
                        break;
                    }
                }
            } else if b.is_ascii_alphabetic() || b == b'_' {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let ident = &text[start..i];

                if ident == "__FILE__" {
                    out.push('"');
                    out.push_str(current_file);
                    out.push('"');
                } else if ident == "__LINE__" {
                    out.push_str(&line_num.to_string());
                } else if !disabled.contains(ident)
                    && let Some(def) = self.macros.get(ident)
                {
                    match def {
                        MacroDef::Object(val) => {
                            let mut next_disabled = disabled.clone();
                            next_disabled.insert(ident.to_string());
                            let expanded_val =
                                self.expand_tokens(val, current_file, line_num, &next_disabled);
                            out.push_str(&expanded_val);
                        }
                        MacroDef::Function {
                            params,
                            body,
                            is_variadic,
                        } => {
                            let mut p = i;
                            while p < bytes.len()
                                && (bytes[p] == b' '
                                    || bytes[p] == b'\t'
                                    || bytes[p] == b'\r'
                                    || bytes[p] == b'\n')
                            {
                                p += 1;
                            }
                            if p < bytes.len() && bytes[p] == b'(' {
                                let mut depth = 1;
                                let mut end_p = p + 1;
                                let mut arg_start = p + 1;
                                let mut raw_args = Vec::new();
                                let mut a_in_str = false;
                                let mut a_in_char = false;
                                let mut a_escape = false;

                                while end_p < bytes.len() && depth > 0 {
                                    let ab = bytes[end_p];
                                    if a_escape {
                                        a_escape = false;
                                    } else if ab == b'\\' {
                                        a_escape = true;
                                    } else if ab == b'"' && !a_in_char {
                                        a_in_str = !a_in_str;
                                    } else if ab == b'\'' && !a_in_str {
                                        a_in_char = !a_in_char;
                                    } else if !a_in_str && !a_in_char {
                                        if ab == b'(' {
                                            depth += 1;
                                        } else if ab == b')' {
                                            depth -= 1;
                                            if depth == 0 {
                                                let arg = text[arg_start..end_p].trim().to_string();
                                                raw_args.push(arg);
                                            }
                                        } else if ab == b',' && depth == 1 {
                                            let arg = text[arg_start..end_p].trim().to_string();
                                            raw_args.push(arg);
                                            arg_start = end_p + 1;
                                        }
                                    }
                                    end_p += 1;
                                }

                                if depth == 0 {
                                    if params.is_empty()
                                        && raw_args.len() == 1
                                        && raw_args[0].is_empty()
                                    {
                                        raw_args.clear();
                                    }

                                    let exp_args: Vec<String> = raw_args
                                        .iter()
                                        .map(|arg| {
                                            self.expand_tokens(
                                                arg,
                                                current_file,
                                                line_num,
                                                disabled,
                                            )
                                        })
                                        .collect();

                                    let mut expanded_body = substitute_macro_params(
                                        body,
                                        params,
                                        &raw_args,
                                        &exp_args,
                                        *is_variadic,
                                    );

                                    expanded_body = replace_token_paste(&expanded_body);

                                    let mut next_disabled = disabled.clone();
                                    next_disabled.insert(ident.to_string());
                                    let final_expanded = self.expand_tokens(
                                        &expanded_body,
                                        current_file,
                                        line_num,
                                        &next_disabled,
                                    );
                                    out.push_str(&final_expanded);
                                    i = end_p;
                                } else {
                                    out.push_str(ident);
                                }
                            } else {
                                out.push_str(ident);
                            }
                        }
                    }
                } else {
                    out.push_str(ident);
                }
            } else {
                out.push(b as char);
                i += 1;
            }
        }

        out
    }

    fn expand_line(&self, line: &str, current_file: &str, line_num: usize) -> String {
        if !line.bytes().any(|b| b.is_ascii_alphabetic() || b == b'_') {
            return line.to_string();
        }
        let disabled = HashSet::new();
        self.expand_tokens(line, current_file, line_num, &disabled)
    }
}

fn substitute_macro_params(
    body: &str,
    params: &[String],
    raw_args: &[String],
    exp_args: &[String],
    is_variadic: bool,
) -> String {
    let mut out = String::with_capacity(body.len());
    let bytes = body.as_bytes();
    let mut i = 0;

    let raw_va_args: Vec<String> = if is_variadic && raw_args.len() > params.len() {
        raw_args[params.len()..].to_vec()
    } else {
        Vec::new()
    };
    let exp_va_args: Vec<String> = if is_variadic && exp_args.len() > params.len() {
        exp_args[params.len()..].to_vec()
    } else {
        Vec::new()
    };
    let raw_va_str = raw_va_args.join(", ");
    let exp_va_str = exp_va_args.join(", ");

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' {
            let quote = b;
            out.push(quote as char);
            i += 1;
            let mut escape = false;
            while i < bytes.len() {
                let c = bytes[i];
                out.push(c as char);
                i += 1;
                if escape {
                    escape = false;
                } else if c == b'\\' {
                    escape = true;
                } else if c == quote {
                    break;
                }
            }
        } else if b == b'#' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'#' {
                out.push('#');
                out.push('#');
                i += 2;
            } else {
                let mut p = i + 1;
                while p < bytes.len()
                    && (bytes[p] == b' '
                        || bytes[p] == b'\t'
                        || bytes[p] == b'\r'
                        || bytes[p] == b'\n')
                {
                    p += 1;
                }
                let mut matched_param_idx = None;
                let mut matched_va = false;
                let mut end_p = p;
                if p < bytes.len() && (bytes[p].is_ascii_alphabetic() || bytes[p] == b'_') {
                    while end_p < bytes.len()
                        && (bytes[end_p].is_ascii_alphanumeric() || bytes[end_p] == b'_')
                    {
                        end_p += 1;
                    }
                    let id = &body[p..end_p];
                    if let Some(pos) = params.iter().position(|param| param == id) {
                        matched_param_idx = Some(pos);
                    } else if is_variadic && id == "__VA_ARGS__" {
                        matched_va = true;
                    }
                }

                if let Some(idx) = matched_param_idx {
                    let arg_val = raw_args.get(idx).map(|s| s.as_str()).unwrap_or("");
                    let escaped = arg_val
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace(['\r', '\n'], " ");
                    out.push('"');
                    out.push_str(&escaped);
                    out.push('"');
                    i = end_p;
                } else if matched_va {
                    let escaped = raw_va_str
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .replace(['\r', '\n'], " ");
                    out.push('"');
                    out.push_str(&escaped);
                    out.push('"');
                    i = end_p;
                } else {
                    out.push('#');
                    i += 1;
                }
            }
        } else if b.is_ascii_alphabetic() || b == b'_' {
            let start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = &body[start..i];

            let param_idx = params.iter().position(|p| p == ident);
            let is_va = is_variadic && ident == "__VA_ARGS__";

            if param_idx.is_some() || is_va {
                let mut prev_is_hash_hash = false;
                let mut p = start;
                while p > 0
                    && (bytes[p - 1] == b' '
                        || bytes[p - 1] == b'\t'
                        || bytes[p - 1] == b'\r'
                        || bytes[p - 1] == b'\n')
                {
                    p -= 1;
                }
                if p >= 2 && bytes[p - 1] == b'#' && bytes[p - 2] == b'#' {
                    prev_is_hash_hash = true;
                }

                let mut next_is_hash_hash = false;
                let mut q = i;
                while q < bytes.len()
                    && (bytes[q] == b' '
                        || bytes[q] == b'\t'
                        || bytes[q] == b'\r'
                        || bytes[q] == b'\n')
                {
                    q += 1;
                }
                if q + 1 < bytes.len() && bytes[q] == b'#' && bytes[q + 1] == b'#' {
                    next_is_hash_hash = true;
                }

                if prev_is_hash_hash || next_is_hash_hash {
                    if let Some(idx) = param_idx {
                        out.push_str(raw_args.get(idx).map(|s| s.as_str()).unwrap_or(""));
                    } else if is_va {
                        out.push_str(&raw_va_str);
                    }
                } else {
                    if let Some(idx) = param_idx {
                        out.push_str(exp_args.get(idx).map(|s| s.as_str()).unwrap_or(""));
                    } else if is_va {
                        out.push_str(&exp_va_str);
                    }
                }
            } else {
                out.push_str(ident);
            }
        } else {
            out.push(b as char);
            i += 1;
        }
    }

    out
}

fn replace_token_paste(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let bytes = src.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'#' && cursor + 1 < bytes.len() && bytes[cursor + 1] == b'#' {
            let mut next_p = cursor + 2;
            while next_p < bytes.len()
                && (bytes[next_p] == b' '
                    || bytes[next_p] == b'\t'
                    || bytes[next_p] == b'\r'
                    || bytes[next_p] == b'\n')
            {
                next_p += 1;
            }
            let temp_out = out.trim_end();
            if temp_out.ends_with(',') && (next_p >= bytes.len() || bytes[next_p] == b')') {
                while out.ends_with(' ')
                    || out.ends_with('\t')
                    || out.ends_with('\r')
                    || out.ends_with('\n')
                {
                    out.pop();
                }
                if out.ends_with(',') {
                    out.pop();
                }
                while out.ends_with(' ')
                    || out.ends_with('\t')
                    || out.ends_with('\r')
                    || out.ends_with('\n')
                {
                    out.pop();
                }
                cursor = next_p;
                continue;
            }

            while out.ends_with(' ')
                || out.ends_with('\t')
                || out.ends_with('\r')
                || out.ends_with('\n')
            {
                out.pop();
            }
            cursor += 2;
            while cursor < bytes.len()
                && (bytes[cursor] == b' '
                    || bytes[cursor] == b'\t'
                    || bytes[cursor] == b'\r'
                    || bytes[cursor] == b'\n')
            {
                cursor += 1;
            }
            continue;
        }
        out.push(bytes[cursor] as char);
        cursor += 1;
    }
    out
}

fn strip_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escape = false;

    while i < bytes.len() {
        let b = bytes[i];

        if escape {
            escape = false;
            out.push(b as char);
            i += 1;
            continue;
        }

        if b == b'\\' && (in_string || in_char) {
            escape = true;
            out.push(b as char);
            i += 1;
            continue;
        }

        if b == b'"' && !in_char {
            in_string = !in_string;
            out.push(b as char);
            i += 1;
            continue;
        }

        if b == b'\'' && !in_string {
            in_char = !in_char;
            out.push(b as char);
            i += 1;
            continue;
        }

        if !in_string && !in_char {
            if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                // Line comment
                i += 2;
                out.push(' ');
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
                continue;
            }

            if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                // Block comment
                i += 2;
                out.push(' ');
                while i < bytes.len() {
                    if bytes[i] == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                        i += 2;
                        break;
                    }
                    if bytes[i] == b'\n' {
                        out.push('\n');
                    } else {
                        out.push(' ');
                    }
                    i += 1;
                }
                continue;
            }
        }

        out.push(b as char);
        i += 1;
    }

    out
}

fn replace_identifier(src: &str, ident: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(src.len() + replacement.len());
    let bytes = src.as_bytes();
    let mut cursor = 0;
    let mut in_str = false;
    let mut in_char = false;
    let mut escape = false;

    while cursor < bytes.len() {
        let b = bytes[cursor];
        if escape {
            escape = false;
            out.push(b as char);
            cursor += 1;
            continue;
        }
        if b == b'\\' {
            escape = true;
            out.push(b as char);
            cursor += 1;
            continue;
        }
        if b == b'"' && !in_char {
            in_str = !in_str;
            out.push(b as char);
            cursor += 1;
            continue;
        }
        if b == b'\'' && !in_str {
            in_char = !in_char;
            out.push(b as char);
            cursor += 1;
            continue;
        }

        if !in_str && !in_char && src[cursor..].starts_with(ident) {
            let before_ok = cursor == 0
                || (!bytes[cursor - 1].is_ascii_alphanumeric() && bytes[cursor - 1] != b'_');
            let after_idx = cursor + ident.len();
            let after_ok = after_idx >= bytes.len()
                || (!bytes[after_idx].is_ascii_alphanumeric() && bytes[after_idx] != b'_');

            if before_ok && after_ok {
                out.push_str(replacement);
                cursor += ident.len();
                continue;
            }
        }

        out.push(b as char);
        cursor += 1;
    }

    out
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum PpToken {
    Num(i64),
    LParen,
    RParen,
    Question,
    Colon,
    LOr,
    LAnd,
    BOr,
    BXor,
    BAnd,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Not,
    BitNot,
}

fn tokenize_pp_expr(src: &str) -> Vec<PpToken> {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if b == b'\'' {
            i += 1;
            let mut val = 0i64;
            if i < bytes.len() && bytes[i] == b'\\' {
                i += 1;
                if i < bytes.len() {
                    val = match bytes[i] {
                        b'n' => b'\n' as i64,
                        b't' => b'\t' as i64,
                        b'r' => b'\r' as i64,
                        b'0' => 0,
                        b'\'' => b'\'' as i64,
                        b'\\' => b'\\' as i64,
                        other => other as i64,
                    };
                    i += 1;
                }
            } else if i < bytes.len() {
                val = bytes[i] as i64;
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'\'' {
                i += 1;
            }
            tokens.push(PpToken::Num(val));
            continue;
        }

        if b.is_ascii_digit() {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            let raw = &src[start..i];
            let num_str = raw.trim_end_matches(['u', 'U', 'l', 'L']);
            let val = if let Some(hex) = num_str
                .strip_prefix("0x")
                .or_else(|| num_str.strip_prefix("0X"))
            {
                i64::from_str_radix(hex, 16).unwrap_or(0)
            } else if let Some(bin) = num_str
                .strip_prefix("0b")
                .or_else(|| num_str.strip_prefix("0B"))
            {
                i64::from_str_radix(bin, 2).unwrap_or(0)
            } else if num_str.len() > 1
                && num_str.starts_with('0')
                && num_str.chars().all(|c| ('0'..='7').contains(&c))
            {
                i64::from_str_radix(num_str, 8).unwrap_or(0)
            } else {
                num_str.parse::<i64>().unwrap_or(0)
            };
            tokens.push(PpToken::Num(val));
            continue;
        }

        match b {
            b'(' => {
                tokens.push(PpToken::LParen);
                i += 1;
            }
            b')' => {
                tokens.push(PpToken::RParen);
                i += 1;
            }
            b'?' => {
                tokens.push(PpToken::Question);
                i += 1;
            }
            b':' => {
                tokens.push(PpToken::Colon);
                i += 1;
            }
            b'~' => {
                tokens.push(PpToken::BitNot);
                i += 1;
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    tokens.push(PpToken::LOr);
                    i += 2;
                } else {
                    tokens.push(PpToken::BOr);
                    i += 1;
                }
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    tokens.push(PpToken::LAnd);
                    i += 2;
                } else {
                    tokens.push(PpToken::BAnd);
                    i += 1;
                }
            }
            b'^' => {
                tokens.push(PpToken::BXor);
                i += 1;
            }
            b'=' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(PpToken::Eq);
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'!' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(PpToken::Ne);
                    i += 2;
                } else {
                    tokens.push(PpToken::Not);
                    i += 1;
                }
            }
            b'<' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'<' {
                    tokens.push(PpToken::Shl);
                    i += 2;
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(PpToken::Le);
                    i += 2;
                } else {
                    tokens.push(PpToken::Lt);
                    i += 1;
                }
            }
            b'>' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'>' {
                    tokens.push(PpToken::Shr);
                    i += 2;
                } else if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(PpToken::Ge);
                    i += 2;
                } else {
                    tokens.push(PpToken::Gt);
                    i += 1;
                }
            }
            b'+' => {
                tokens.push(PpToken::Add);
                i += 1;
            }
            b'-' => {
                tokens.push(PpToken::Sub);
                i += 1;
            }
            b'*' => {
                tokens.push(PpToken::Mul);
                i += 1;
            }
            b'/' => {
                tokens.push(PpToken::Div);
                i += 1;
            }
            b'%' => {
                tokens.push(PpToken::Mod);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    tokens
}

struct PpParser<'a> {
    tokens: &'a [PpToken],
    pos: usize,
}

impl<'a> PpParser<'a> {
    fn peek(&self) -> Option<&PpToken> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<&PpToken> {
        let tok = self.tokens.get(self.pos);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn parse_expr(&mut self) -> i64 {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> i64 {
        let cond = self.parse_lor();
        if let Some(PpToken::Question) = self.peek() {
            self.advance();
            let then_val = self.parse_expr();
            if let Some(PpToken::Colon) = self.peek() {
                self.advance();
            }
            let else_val = self.parse_ternary();
            if cond != 0 { then_val } else { else_val }
        } else {
            cond
        }
    }

    fn parse_lor(&mut self) -> i64 {
        let mut left = self.parse_land();
        while let Some(PpToken::LOr) = self.peek() {
            self.advance();
            let right = self.parse_land();
            left = if left != 0 || right != 0 { 1 } else { 0 };
        }
        left
    }

    fn parse_land(&mut self) -> i64 {
        let mut left = self.parse_bor();
        while let Some(PpToken::LAnd) = self.peek() {
            self.advance();
            let right = self.parse_bor();
            left = if left != 0 && right != 0 { 1 } else { 0 };
        }
        left
    }

    fn parse_bor(&mut self) -> i64 {
        let mut left = self.parse_bxor();
        while let Some(PpToken::BOr) = self.peek() {
            self.advance();
            let right = self.parse_bxor();
            left |= right;
        }
        left
    }

    fn parse_bxor(&mut self) -> i64 {
        let mut left = self.parse_band();
        while let Some(PpToken::BXor) = self.peek() {
            self.advance();
            let right = self.parse_band();
            left ^= right;
        }
        left
    }

    fn parse_band(&mut self) -> i64 {
        let mut left = self.parse_eq();
        while let Some(PpToken::BAnd) = self.peek() {
            self.advance();
            let right = self.parse_eq();
            left &= right;
        }
        left
    }

    fn parse_eq(&mut self) -> i64 {
        let mut left = self.parse_rel();
        while let Some(tok) = self.peek() {
            match tok {
                PpToken::Eq => {
                    self.advance();
                    let right = self.parse_rel();
                    left = if left == right { 1 } else { 0 };
                }
                PpToken::Ne => {
                    self.advance();
                    let right = self.parse_rel();
                    left = if left != right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_rel(&mut self) -> i64 {
        let mut left = self.parse_shift();
        while let Some(tok) = self.peek() {
            match tok {
                PpToken::Lt => {
                    self.advance();
                    let right = self.parse_shift();
                    left = if left < right { 1 } else { 0 };
                }
                PpToken::Le => {
                    self.advance();
                    let right = self.parse_shift();
                    left = if left <= right { 1 } else { 0 };
                }
                PpToken::Gt => {
                    self.advance();
                    let right = self.parse_shift();
                    left = if left > right { 1 } else { 0 };
                }
                PpToken::Ge => {
                    self.advance();
                    let right = self.parse_shift();
                    left = if left >= right { 1 } else { 0 };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_shift(&mut self) -> i64 {
        let mut left = self.parse_add();
        while let Some(tok) = self.peek() {
            match tok {
                PpToken::Shl => {
                    self.advance();
                    let right = self.parse_add();
                    left = left.wrapping_shl(right as u32);
                }
                PpToken::Shr => {
                    self.advance();
                    let right = self.parse_add();
                    left = left.wrapping_shr(right as u32);
                }
                _ => break,
            }
        }
        left
    }

    fn parse_add(&mut self) -> i64 {
        let mut left = self.parse_mul();
        while let Some(tok) = self.peek() {
            match tok {
                PpToken::Add => {
                    self.advance();
                    let right = self.parse_mul();
                    left = left.wrapping_add(right);
                }
                PpToken::Sub => {
                    self.advance();
                    let right = self.parse_mul();
                    left = left.wrapping_sub(right);
                }
                _ => break,
            }
        }
        left
    }

    fn parse_mul(&mut self) -> i64 {
        let mut left = self.parse_unary();
        while let Some(tok) = self.peek() {
            match tok {
                PpToken::Mul => {
                    self.advance();
                    let right = self.parse_unary();
                    left = left.wrapping_mul(right);
                }
                PpToken::Div => {
                    self.advance();
                    let right = self.parse_unary();
                    left = if right != 0 {
                        left.wrapping_div(right)
                    } else {
                        0
                    };
                }
                PpToken::Mod => {
                    self.advance();
                    let right = self.parse_unary();
                    left = if right != 0 {
                        left.wrapping_rem(right)
                    } else {
                        0
                    };
                }
                _ => break,
            }
        }
        left
    }

    fn parse_unary(&mut self) -> i64 {
        match self.peek() {
            Some(PpToken::Not) => {
                self.advance();
                let v = self.parse_unary();
                if v == 0 { 1 } else { 0 }
            }
            Some(PpToken::BitNot) => {
                self.advance();
                !self.parse_unary()
            }
            Some(PpToken::Add) => {
                self.advance();
                self.parse_unary()
            }
            Some(PpToken::Sub) => {
                self.advance();
                self.parse_unary().wrapping_neg()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> i64 {
        match self.advance() {
            Some(PpToken::Num(v)) => *v,
            Some(PpToken::LParen) => {
                let v = self.parse_expr();
                if let Some(PpToken::RParen) = self.peek() {
                    self.advance();
                }
                v
            }
            _ => 0,
        }
    }
}

fn eval_pp_const_expr(src: &str) -> i64 {
    let tokens = tokenize_pp_expr(src);
    let mut parser = PpParser {
        tokens: &tokens,
        pos: 0,
    };
    parser.parse_expr()
}
