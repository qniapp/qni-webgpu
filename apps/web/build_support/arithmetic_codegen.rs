use std::{collections::BTreeMap, fs, path::Path};
use syn::{BinOp, Expr, FnArg, Item, Pat, ReturnType, Stmt, Type, UnOp};

pub const MARKER: &str = "// GENERATED_ARITHMETIC_INDEX";
type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    U32,
    Bool,
}

fn unsupported<T>(what: &str) -> Result<T> {
    Err(format!("unsupported arithmetic syntax: {what}"))
}

fn u32_type(ty: &Type) -> bool {
    matches!(ty, Type::Path(p) if p.qself.is_none() && p.path.is_ident("u32"))
}

fn binding(pat: &Pat) -> Result<String> {
    match pat {
        Pat::Ident(p)
            if p.attrs.is_empty()
                && p.by_ref.is_none()
                && p.mutability.is_none()
                && p.subpat.is_none() =>
        {
            Ok(p.ident.to_string())
        }
        _ => unsupported("immutable simple binding required"),
    }
}

struct Emitter {
    names: BTreeMap<String, String>,
}

impl Emitter {
    fn bind(&mut self, name: String, output: String) -> Result<()> {
        if self.names.insert(name, output).is_some() {
            return unsupported("shadowing or duplicate binding");
        }
        Ok(())
    }

    fn u32_expr(&self, expr: &Expr) -> Result<String> {
        let (text, kind) = self.expr(expr)?;
        if kind != Kind::U32 {
            return unsupported("expected u32 expression");
        }
        Ok(text)
    }

    fn expr(&self, expr: &Expr) -> Result<(String, Kind)> {
        let text = match expr {
            Expr::Path(p)
                if p.attrs.is_empty() && p.qself.is_none() && p.path.get_ident().is_some() =>
            {
                self.names
                    .get(&p.path.get_ident().unwrap().to_string())
                    .cloned()
                    .ok_or("unsupported arithmetic syntax: unknown identifier")?
            }
            Expr::Paren(p) if p.attrs.is_empty() => {
                let (text, kind) = self.expr(&p.expr)?;
                return Ok((format!("({text})"), kind));
            }
            Expr::Lit(l) if l.attrs.is_empty() => match &l.lit {
                syn::Lit::Int(n) if n.suffix() == "u32" => {
                    let value = n.base10_parse::<u32>().map_err(|_| "invalid u32 literal")?;
                    format!("{value}u")
                }
                _ => return unsupported("u32 literal required"),
            },
            Expr::Unary(u) if u.attrs.is_empty() && matches!(u.op, UnOp::Not(_)) => {
                format!("(~{})", self.u32_expr(&u.expr)?)
            }
            Expr::Binary(b) if b.attrs.is_empty() => {
                let (op, kind) = match b.op {
                    BinOp::BitAnd(_) => ("&", Kind::U32),
                    BinOp::BitOr(_) => ("|", Kind::U32),
                    BinOp::Shl(_) => ("<<", Kind::U32),
                    BinOp::Shr(_) => (">>", Kind::U32),
                    BinOp::Eq(_) => ("==", Kind::Bool),
                    BinOp::Ne(_) => ("!=", Kind::Bool),
                    _ => return unsupported("binary operator"),
                };
                let left = self.u32_expr(&b.left)?;
                let right = self.u32_expr(&b.right)?;
                return Ok((format!("({left} {op} {right})"), kind));
            }
            Expr::MethodCall(m)
                if m.attrs.is_empty()
                    && m.method == "wrapping_sub"
                    && m.turbofish.is_none()
                    && m.args.len() == 1 =>
            {
                format!(
                    "({} - {})",
                    self.u32_expr(&m.receiver)?,
                    self.u32_expr(&m.args[0])?
                )
            }
            _ => return unsupported("expression"),
        };
        Ok((text, Kind::U32))
    }
}

pub fn translate(source: &str) -> Result<String> {
    let file = syn::parse_file(source).map_err(|e| e.to_string())?;
    if !file.attrs.is_empty() || file.shebang.is_some() || file.items.len() != 1 {
        return unsupported("exactly one function required");
    }
    let Item::Fn(f) = &file.items[0] else {
        return unsupported("function required");
    };
    let sig = &f.sig;
    if !f.attrs.is_empty()
        || sig.ident != "source_index"
        || sig.constness.is_some()
        || sig.asyncness.is_some()
        || !matches!(sig.safety, syn::Safety::Default)
        || sig.abi.is_some()
        || sig.variadic.is_some()
        || !sig.generics.params.is_empty()
        || sig.generics.where_clause.is_some()
        || sig.inputs.len() != 6
        || !matches!(&sig.output, ReturnType::Type(_, t) if u32_type(t))
        || !matches!(&f.vis, syn::Visibility::Restricted(v)
            if v.in_token.is_none() && v.path.is_ident("crate"))
    {
        return unsupported("source_index signature");
    }
    let mut emitter = Emitter {
        names: BTreeMap::new(),
    };
    let mut args = Vec::new();
    for (index, arg) in sig.inputs.iter().enumerate() {
        let FnArg::Typed(arg) = arg else {
            return unsupported("argument");
        };
        if !arg.attrs.is_empty() || !u32_type(&arg.ty) {
            return unsupported("u32 argument");
        }
        let name = format!("arg{index}");
        emitter.bind(binding(&arg.pat)?, name.clone())?;
        args.push(format!("{name}: u32"));
    }
    let mut output = format!("fn source_index({}) -> u32 {{\n", args.join(", "));
    let stmts = &f.block.stmts;
    if stmts.len() < 2 {
        return unsupported("control guard and final expression required");
    }
    let Stmt::Expr(Expr::If(guard), None) = &stmts[0] else {
        return unsupported("initial control guard required");
    };
    if !guard.attrs.is_empty() || guard.else_branch.is_some() || guard.then_branch.stmts.len() != 1
    {
        return unsupported("early return guard");
    }
    let (condition, kind) = emitter.expr(&guard.cond)?;
    if kind != Kind::Bool {
        return unsupported("bool condition required");
    }
    let Stmt::Expr(Expr::Return(ret), Some(_)) = &guard.then_branch.stmts[0] else {
        return unsupported("early return required");
    };
    if !ret.attrs.is_empty() {
        return unsupported("return attributes");
    }
    let returned = emitter.u32_expr(ret.expr.as_deref().ok_or("missing return value")?)?;
    output.push_str(&format!("    if ({condition}) {{ return {returned}; }}\n"));
    for (index, stmt) in stmts[1..stmts.len() - 1].iter().enumerate() {
        let Stmt::Local(local) = stmt else {
            return unsupported("initialized local required");
        };
        if !local.attrs.is_empty() {
            return unsupported("local attributes");
        }
        let pat = match &local.pat {
            Pat::Type(t) if t.attrs.is_empty() && u32_type(&t.ty) => &*t.pat,
            p => p,
        };
        let name = binding(pat)?;
        let init = local.init.as_ref().ok_or("missing local initializer")?;
        if init.diverge.is_some() {
            return unsupported("let else");
        }
        let value = emitter.u32_expr(&init.expr)?;
        let safe_name = format!("local{index}");
        emitter.bind(name, safe_name.clone())?;
        output.push_str(&format!("    let {safe_name}: u32 = {value};\n"));
    }
    let Stmt::Expr(tail, None) = &stmts[stmts.len() - 1] else {
        return unsupported("final expression required");
    };
    output.push_str(&format!("    return {};\n}}\n", emitter.u32_expr(tail)?));
    Ok(output)
}

pub fn assemble(source: &str, shell: &str) -> Result<String> {
    if shell.matches(MARKER).count() != 1 {
        return unsupported("exactly one generated-helper marker required");
    }
    Ok(shell.replace(MARKER, &translate(source)?))
}

pub fn generate(manifest: &Path, out: &Path) {
    let source = "src/gpu/arithmetic_index.rs";
    let shell = "src/gpu/shaders/arithmetic.wgsl";
    for path in [source, shell, "build_support/arithmetic_codegen.rs"] {
        println!("cargo:rerun-if-changed={}", manifest.join(path).display());
    }
    let shader = assemble(
        &fs::read_to_string(manifest.join(source)).expect("read arithmetic Rust"),
        &fs::read_to_string(manifest.join(shell)).expect("read arithmetic shell"),
    )
    .unwrap_or_else(|e| panic!("arithmetic WGSL generation failed: {e}"));
    fs::write(out.join("arithmetic.wgsl"), shader).expect("write arithmetic WGSL");
}
