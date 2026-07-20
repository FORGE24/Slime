mod ast;
mod codegen;
mod ctfe;
mod diag;
mod dope;
mod dynamic_precomp;
mod etca;
#[allow(dead_code)]
mod heritage;
mod ifm;
mod interp;
mod lexer;
mod link;
mod mono;
mod ownck;
mod parser;
mod pre_concurrency;
mod scheduler_elimination;
mod tce;

use std::env;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::Instant;

use codegen::Codegen;
use diag::{emit_plain_error, set_color, SourceFile};
use etca::EtcaOpts;
use interp::Interp;
use lexer::Lexer;
use parser::Parser;

fn main() {
    set_color(std::io::stderr().is_terminal());

    if let Err(()) = run() {
        process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let mut args = env::args().skip(1);
    let Some(input) = args.next() else {
        print_usage();
        emit_plain_error("missing input file");
        return Err(());
    };

    let mut emit_path: Option<String> = None;
    let mut dump_ast = false;
    let mut color_flag: Option<bool> = None;
    let mut quiet = false;
    let mut show_time = false;
    let mut do_run = false;
    let mut use_llvm_run = false;
    let mut opt_aot = false; // -O: CTFE residual + clang -O3 (faster than runtime C)
    let mut opts = EtcaOpts::default();
    opts.disable_all();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" => {
                let Some(path) = args.next() else {
                    emit_plain_error("missing path after `-o`");
                    return Err(());
                };
                emit_path = Some(path);
            }
            "--dump-ast" => dump_ast = true,
            "--color" => color_flag = Some(true),
            "--no-color" => color_flag = Some(false),
            "-q" | "--quiet" => quiet = true,
            "--time" => show_time = true,
            "-r" | "--run" | "-fc" => do_run = true,
            "--llvm" => use_llvm_run = true,
            "-O" | "--optimize" => {
                opt_aot = true;
                do_run = true;
            }
            "--etca" => {
                opts.ctfe = true;
                opts.dope = true;
                opts.precomp = true;
                opts.tce = true;
                opts.ifm = true;
                opts.scheduler = true;
                opts.pre_concurrency = true;
            }
            "--ctfe" => opts.ctfe = true,
            "--no-ctfe" => opts.ctfe = false,
            "--dope" => opts.dope = true,
            "--no-dope" => opts.dope = false,
            "--precomp" => opts.precomp = true,
            "--no-precomp" => opts.precomp = false,
            "--tce" => opts.tce = true,
            "--no-tce" => opts.tce = false,
            "--ifm" => opts.ifm = true,
            "--no-ifm" => opts.ifm = false,
            "--scheduler" => opts.scheduler = true,
            "--no-scheduler" => opts.scheduler = false,
            "--pre-concurrency" => opts.pre_concurrency = true,
            "--no-pre-concurrency" => opts.pre_concurrency = false,
            "--no-etca" => opts.disable_all(),
            "--ctfe-report" | "--etca-report" => {
                // Report implies a full ETCA pass (otherwise nothing runs).
                opts.ctfe = true;
                opts.dope = true;
                opts.precomp = true;
                opts.tce = true;
                opts.ifm = true;
                opts.scheduler = true;
                opts.pre_concurrency = true;
                opts.report = true;
            }
            "-h" | "--help" => {
                print_usage();
                return Ok(());
            }
            other => {
                emit_plain_error(&format!("unknown argument: `{other}`"));
                return Err(());
            }
        }
    }
    if let Some(c) = color_flag {
        set_color(c);
    }

    let t0 = Instant::now();

    let src = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            emit_plain_error(&format!("couldn't read `{input}`: {e}"));
            return Err(());
        }
    };
    let file = SourceFile::new(&input, src);

    let t_lex0 = Instant::now();
    let tokens = match Lexer::new(&file.src).tokenize() {
        Ok(t) => t,
        Err(d) => {
            d.emit(&file);
            return Err(());
        }
    };
    let t_lex = t_lex0.elapsed();

    let t_parse0 = Instant::now();
    let mut program = match Parser::new(tokens).parse() {
        Ok(p) => p,
        Err(d) => {
            d.emit(&file);
            return Err(());
        }
    };
    if let Err(d) = link::resolve_links(&mut program, Path::new(&input)) {
        d.emit(&file);
        return Err(());
    }
    let t_parse = t_parse0.elapsed();

    if let Err(d) = mono::monomorphize(&mut program) {
        d.emit(&file);
        return Err(());
    }

    if let Err(d) = ownck::check(&program) {
        d.emit(&file);
        return Err(());
    }

    if dump_ast {
        println!("{program:#?}");
    }

    // -O: compute at compile time → residual native exe (I/O only) → faster than runtime C
    if opt_aot {
        let interp = Interp::new(&program);
        let t_pe0 = Instant::now();
        let lines = match interp.run_main_capture() {
            Ok(l) => l,
            Err(d) => {
                d.emit(&file);
                return Err(());
            }
        };
        let t_pe = t_pe0.elapsed();
        let ir = interp::emit_residual_ll(&lines);
        let out = emit_path.unwrap_or_else(|| {
            Path::new(&input)
                .with_extension("ll")
                .to_string_lossy()
                .into_owned()
        });
        if let Err(e) = fs::write(&out, &ir) {
            emit_plain_error(&format!("couldn't write `{out}`: {e}"));
            return Err(());
        }
        let t_clang0 = Instant::now();
        let exe = run_via_clang(Path::new(&out), Path::new(&input), "3")?;
        let t_clang = t_clang0.elapsed();
        let t_run0 = Instant::now();
        let status = Command::new(&exe).status().map_err(|e| {
            emit_plain_error(&format!("couldn't execute `{}`: {e}", exe.display()));
        })?;
        let t_run = t_run0.elapsed();
        if show_time {
            eprintln!(
                "time: lex={:.2}ms parse={:.2}ms pe={:.2}ms clang={:.2}ms run={:.2}ms wall={:.2}ms (residual AOT)",
                t_lex.as_secs_f64() * 1000.0,
                t_parse.as_secs_f64() * 1000.0,
                t_pe.as_secs_f64() * 1000.0,
                t_clang.as_secs_f64() * 1000.0,
                t_run.as_secs_f64() * 1000.0,
                t0.elapsed().as_secs_f64() * 1000.0,
            );
        }
        if !quiet {
            eprintln!("wrote {} + {} (compile-time folded)", out, exe.display());
        }
        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    // Fast path: interpret (Fox-like).
    if do_run && !use_llvm_run && emit_path.is_none() {
        let t_run0 = Instant::now();
        if let Err(d) = Interp::new(&program).run_main() {
            d.emit(&file);
            return Err(());
        }
        let t_run = t_run0.elapsed();
        if show_time {
            eprintln!(
                "time: lex={:.2}ms parse={:.2}ms interp={:.2}ms wall={:.2}ms",
                t_lex.as_secs_f64() * 1000.0,
                t_parse.as_secs_f64() * 1000.0,
                t_run.as_secs_f64() * 1000.0,
                t0.elapsed().as_secs_f64() * 1000.0,
            );
        }
        return Ok(());
    }

    let t_cg0 = Instant::now();
    let (ir, etca) = match Codegen::compile(&program, opts) {
        Ok(x) => x,
        Err(d) => {
            d.emit(&file);
            return Err(());
        }
    };
    let t_cg = t_cg0.elapsed();

    if let Some(mut etca) = etca {
        etca.finalize_scheduler();
        if !quiet {
            etca.emit_summary();
        }
        if opts.report {
            etca.emit_reports();
        }
    }

    let out = emit_path.unwrap_or_else(|| {
        Path::new(&input)
            .with_extension("ll")
            .to_string_lossy()
            .into_owned()
    });
    if let Err(e) = fs::write(&out, &ir) {
        emit_plain_error(&format!("couldn't write `{out}`: {e}"));
        return Err(());
    }
    if !quiet && !do_run {
        eprintln!("wrote {out}");
    }

    let t_compile = t0.elapsed();

    if do_run && use_llvm_run {
        let t_clang0 = Instant::now();
        let exe = run_via_clang(Path::new(&out), Path::new(&input), "3")?;
        let t_clang = t_clang0.elapsed();

        let t_run0 = Instant::now();
        let status = Command::new(&exe).status().map_err(|e| {
            emit_plain_error(&format!("couldn't execute `{}`: {e}", exe.display()));
        })?;
        let t_run = t_run0.elapsed();

        if show_time {
            eprintln!(
                "time: lex={:.2}ms parse={:.2}ms codegen={:.2}ms clang={:.2}ms run={:.2}ms wall={:.2}ms",
                t_lex.as_secs_f64() * 1000.0,
                t_parse.as_secs_f64() * 1000.0,
                t_cg.as_secs_f64() * 1000.0,
                t_clang.as_secs_f64() * 1000.0,
                t_run.as_secs_f64() * 1000.0,
                t0.elapsed().as_secs_f64() * 1000.0,
            );
        }

        if !status.success() {
            process::exit(status.code().unwrap_or(1));
        }
        return Ok(());
    }

    if show_time {
        eprintln!(
            "time: lex={:.2}ms parse={:.2}ms codegen={:.2}ms total={:.2}ms",
            t_lex.as_secs_f64() * 1000.0,
            t_parse.as_secs_f64() * 1000.0,
            t_cg.as_secs_f64() * 1000.0,
            t_compile.as_secs_f64() * 1000.0,
        );
    }

    Ok(())
}

fn run_via_clang(ll: &Path, src: &Path, opt: &str) -> Result<PathBuf, ()> {
    let clang = find_clang().ok_or_else(|| {
        emit_plain_error("clang not found (needed for `-O` / `--llvm`); install LLVM or set CLANG=");
    })?;

    let exe = src.with_extension(if cfg!(windows) { "exe" } else { "out" });
    let mut cmd = Command::new(&clang);
    cmd.arg(ll).arg(format!("-O{opt}")).arg("-Wno-override-module");
    if let Some(rt) = find_slime_rt() {
        cmd.arg(&rt);
    }
    cmd.arg("-o").arg(&exe);
    let status = cmd.status().map_err(|e| {
        emit_plain_error(&format!("failed to spawn clang (`{}`): {e}", clang.display()));
    })?;

    if !status.success() {
        emit_plain_error("clang failed to link executable");
        return Err(());
    }
    Ok(exe)
}

fn find_slime_rt() -> Option<PathBuf> {
    let mut cands = Vec::new();
    if let Ok(exe) = env::current_exe() {
        if let Some(dir) = exe.parent() {
            cands.push(dir.join("rt").join("slime_rt.c"));
            cands.push(dir.join("..").join("rt").join("slime_rt.c"));
        }
    }
    cands.push(PathBuf::from("rt/slime_rt.c"));
    cands.push(PathBuf::from("bench/slime_rt.c"));
    cands.into_iter().find(|p| p.is_file())
}

fn find_clang() -> Option<PathBuf> {
    if let Ok(p) = env::var("CLANG") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    for cmd in ["clang", "clang.exe"] {
        if let Some(p) = which(cmd) {
            return Some(p);
        }
    }
    for c in [
        r"D:\Program Files\LLVM\bin\clang.exe",
        r"C:\Program Files\LLVM\bin\clang.exe",
        r"C:\Program Files (x86)\LLVM\bin\clang.exe",
    ] {
        let p = PathBuf::from(c);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn which(cmd: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let p = dir.join(cmd);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn print_usage() {
    eprintln!(
        "\
slime2 — Slime language (interpreter + LLVM AOT + compile-time fold)

Usage:
  slime <file.sm> -fc          # interpret
  slime <file.sm> -O           # whole-program residual AOT (beats runtime C)
  slime <file.sm> -o out.ll    # emit full LLVM IR
  slime <file.sm> --etca -o out.ll   # CTFE2: fold pure calls → IR constants

  # After -O, re-run the residual exe alone (I/O only, no MT/fib work):
  #   .\\examples\\mt_fib.exe

Speed:
  -O / --optimize  compute at compile-time, emit print-only native binary
  --etca / --ctfe  enable ETCA (function fold + const collapse into IR)
  -fc / -r         interpret
  --llvm -r        full IR + clang (use -O instead to beat C)
  --time           phase timings
  cargo build --release

Upstream: https://github.com/FORGE24/Slime"
    );
}
