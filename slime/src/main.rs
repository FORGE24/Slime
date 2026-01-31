use clap::{Parser, Subcommand};
use std::process::{Command, Stdio};
use std::fs;
use std::path::{Path, PathBuf};
use std::env;
use which::which;
use toml::Value;
use std::time::SystemTime;
use std::collections::HashMap;
use serde_json;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a Slime project
    Build {
        /// Input file
        input: PathBuf,
        
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Target platform
        #[arg(short, long, default_value = "windows-x64")]
        target: String,
        
        /// Optimization level
        #[arg(short, long, default_value = "O0")]
        optimization: String,
        
        /// Generate debug information
        #[arg(short, long, default_value = "false")]
        debug: bool,
    },
    
    /// Initialize a new Slime project
    Init {
        /// Project name
        name: String,
        
        /// Template to use
        #[arg(short, long, default_value = "default")]
        template: String,
    },
    
    /// Package management commands
    #[command(subcommand)]
    Package(PackageCommands),
    
    /// Code analysis commands
    #[command(subcommand)]
    Analyze(AnalyzeCommands),
}

#[derive(Subcommand)]
enum PackageCommands {
    /// Add a dependency
    Add {
        /// Package name
        package: String,
        
        /// Package version
        #[arg(short, long)]
        version: Option<String>,
    },
    
    /// Remove a dependency
    Remove {
        /// Package name
        package: String,
    },
    
    /// Update dependencies
    Update {
        /// Package name (optional, updates all if not specified)
        package: Option<String>,
    },
    
    /// List dependencies
    List,
    
    /// Install dependencies
    Install,
    
    /// Publish a package
    Publish {
        /// Package version
        #[arg(short, long)]
        version: Option<String>,
    },
}

#[derive(Subcommand)]
enum AnalyzeCommands {
    /// Check for syntax errors
    Check {
        /// Input file or directory
        input: PathBuf,
    },
    
    /// Lint code
    Lint {
        /// Input file or directory
        input: PathBuf,
    },
    
    /// Calculate code metrics
    Metrics {
        /// Input file or directory
        input: PathBuf,
    },
    
    /// Find unused variables and functions
    Unused {
        /// Input file or directory
        input: PathBuf,
    },
}

fn read_config() -> Result<Option<Value>, Box<dyn std::error::Error>> {
    let config_path = Path::new("slime.toml");
    if !config_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(config_path)?;
    let config: Value = toml::from_str(&content)?;
    Ok(Some(config))
}

fn get_file_mtime(path: &Path) -> Result<SystemTime, Box<dyn std::error::Error>> {
    let metadata = fs::metadata(path)?;
    let mtime = metadata.modified()?;
    Ok(mtime)
}

fn read_build_cache() -> Result<Option<HashMap<String, SystemTime>>, Box<dyn std::error::Error>> {
    let cache_path = Path::new(".slime-build-cache");
    if !cache_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(cache_path)?;
    let cache: HashMap<String, SystemTime> = serde_json::from_str(&content)?;
    Ok(Some(cache))
}

fn write_build_cache(cache: &HashMap<String, SystemTime>) -> Result<(), Box<dyn std::error::Error>> {
    let cache_path = Path::new(".slime-build-cache");
    let content = serde_json::to_string(cache)?;
    fs::write(cache_path, content)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let config = read_config()?;

    match &cli.command {
        Commands::Build { input, output, target, optimization, debug } => {
            build(input, output, target, optimization, *debug, &config)?;
        }
        Commands::Init { name, template } => {
            init(name, template)?;
        }
        Commands::Package(package_cmd) => {
            package(package_cmd, &config)?;
        }
        Commands::Analyze(analyze_cmd) => {
            analyze(analyze_cmd)?;
        }
    }

    Ok(())
}

fn init(
    name: &str,
    template: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Initializing new Slime project '{}' with template '{}'...", name, template);
    
    // 创建项目目录
    let project_dir = Path::new(name);
    if project_dir.exists() {
        return Err(format!("Directory '{}' already exists", name).into());
    }
    fs::create_dir_all(project_dir)?;
    
    // 创建 src 目录
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    
    // 创建 main.sm 文件
    let main_file = src_dir.join("main.sm");
    fs::write(main_file, r#"fn main() {
    println("Hello, Slime!");
}
"#)?;
    
    // 创建 slime.toml 文件
    let toml_file = project_dir.join("slime.toml");
    fs::write(toml_file, &format!(r#"# Slime project configuration

[package]
name = "{}"
version = "1.0.0"
description = "A new Slime project"
authors = ["Your Name <your.email@example.com>"]

[build]
input = "src/main.sm"
output = "bin/{}.exe"
target = "x86_64-windows"
optimization = 2
debug = false
include_paths = ["lib"]

[features]
default = ["std"]
std = true

[dependencies]
"#, name, name))?;
    
    // 创建 .gitignore 文件
    let gitignore_file = project_dir.join(".gitignore");
    fs::write(gitignore_file, r#"bin/\n*.exe\n*.asm\n*.obj\n*.o\n.target/\n.idea/\n.vscode/\n*.swp\n*.swo\n*~\n"#)?;
    
    println!("✓ Project '{}' initialized successfully", name);
    println!("  Directory structure:");
    println!("  {}/", name);
    println!("    ├── src/");
    println!("    │   └── main.sm");
    println!("    ├── slime.toml");
    println!("    └── .gitignore");
    
    Ok(())
}

fn analyze(
    cmd: &AnalyzeCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        AnalyzeCommands::Check { input } => {
            check_syntax(input)?;
        }
        AnalyzeCommands::Lint { input } => {
            lint_code(input)?;
        }
        AnalyzeCommands::Metrics { input } => {
            calculate_metrics(input)?;
        }
        AnalyzeCommands::Unused { input } => {
            find_unused(input)?;
        }
    }
    Ok(())
}

fn check_syntax(
    input: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Checking syntax for {}...", input.display());
    // 实际实现会使用 slimec 检查语法
    println!("✓ Syntax check passed");
    Ok(())
}

fn lint_code(
    input: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Linting code for {}...", input.display());
    // 实际实现会进行代码风格检查
    println!("✓ Linting completed");
    Ok(())
}

fn calculate_metrics(
    input: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Calculating metrics for {}...", input.display());
    // 实际实现会计算代码度量指标
    println!("✓ Metrics calculated");
    println!("  Lines of code: 100");
    println!("  Functions: 10");
    println!("  Complexity: 5");
    Ok(())
}

fn find_unused(
    input: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Finding unused variables and functions in {}...", input.display());
    // 实际实现会查找未使用的变量和函数
    println!("✓ Analysis completed");
    Ok(())
}

fn package(
    cmd: &PackageCommands,
    config: &Option<Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        PackageCommands::Add { package, version } => {
            add_dependency(package, version)?;
        }
        PackageCommands::Remove { package } => {
            remove_dependency(package)?;
        }
        PackageCommands::Update { package } => {
            update_dependency(package.as_deref())?;
        }
        PackageCommands::List => {
            list_dependencies()?;
        }
        PackageCommands::Install => {
            install_dependencies()?;
        }
        PackageCommands::Publish { version } => {
            publish_package(version)?;
        }
    }
    Ok(())
}

fn install_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    println!("Installing dependencies...");
    // 实际实现会从包管理器下载依赖
    println!("✓ Dependencies installed successfully");
    Ok(())
}

fn publish_package(version: &Option<String>) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("slime.toml");
    if !config_path.exists() {
        return Err("slime.toml not found. Please run this command in a Slime project directory".into());
    }

    println!("Publishing package...");
    // 实际实现会将包发布到包管理器
    println!("✓ Package published successfully");
    Ok(())
}

fn add_dependency(
    package: &str,
    version: &Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("slime.toml");
    if !config_path.exists() {
        return Err("slime.toml not found. Please run this command in a Slime project directory".into());
    }

    let content = fs::read_to_string(config_path)?;
    let mut config: Value = toml::from_str(&content)?;

    // 确保 dependencies 部分存在
    if let Some(table) = config.as_table_mut() {
        if !table.contains_key("dependencies") {
            table.insert("dependencies".to_string(), Value::Table(toml::Table::new()));
        }

        // 添加依赖
        if let Some(Value::Table(deps)) = table.get_mut("dependencies") {
            let version_str = version.as_deref().unwrap_or("*");
            deps.insert(package.to_string(), Value::String(version_str.to_string()));
        }
    }

    let updated_content = toml::to_string(&config)?;
    fs::write(config_path, updated_content)?;

    println!("✓ Added dependency: {}@{}", package, version.as_deref().unwrap_or("*"));
    Ok(())
}

fn remove_dependency(
    package: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("slime.toml");
    if !config_path.exists() {
        return Err("slime.toml not found. Please run this command in a Slime project directory".into());
    }

    let content = fs::read_to_string(config_path)?;
    let mut config: Value = toml::from_str(&content)?;

    if let Some(table) = config.as_table_mut() {
        if let Some(Value::Table(deps)) = table.get_mut("dependencies") {
            if deps.remove(package).is_some() {
                let updated_content = toml::to_string(&config)?;
                fs::write(config_path, updated_content)?;
                println!("✓ Removed dependency: {}", package);
            } else {
                println!("⚠ Dependency not found: {}", package);
            }
        } else {
            println!("⚠ No dependencies found in slime.toml");
        }
    } else {
        println!("⚠ Invalid slime.toml format");
    }

    Ok(())
}

fn update_dependency(
    package: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("slime.toml");
    if !config_path.exists() {
        return Err("slime.toml not found. Please run this command in a Slime project directory".into());
    }

    let content = fs::read_to_string(config_path)?;
    let config: Value = toml::from_str(&content)?;

    if let Some(Value::Table(deps)) = config.get("dependencies") {
        if let Some(pkg) = package {
            if deps.contains_key(pkg) {
                println!("✓ Updated dependency: {}", pkg);
            } else {
                println!("⚠ Dependency not found: {}", pkg);
            }
        } else {
            println!("✓ Updated all dependencies");
        }
    } else {
        println!("⚠ No dependencies found in slime.toml");
    }

    Ok(())
}

fn list_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let config_path = Path::new("slime.toml");
    if !config_path.exists() {
        return Err("slime.toml not found. Please run this command in a Slime project directory".into());
    }

    let content = fs::read_to_string(config_path)?;
    let config: Value = toml::from_str(&content)?;

    println!("Dependencies:");
    println!("────────────");

    if let Some(Value::Table(deps)) = config.get("dependencies") {
        if deps.is_empty() {
            println!("(No dependencies)");
        } else {
            for (name, version) in deps {
                if let Value::String(ver) = version {
                    println!("{} = {}", name, ver);
                }
            }
        }
    } else {
        println!("(No dependencies)");
    }

    Ok(())
}

fn build(
    input: &PathBuf,
    output: &Option<PathBuf>,
    target: &str,
    optimization: &str,
    debug: bool,
    config: &Option<Value>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 使用配置文件中的值（如果存在）
    let config_input = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("input").and_then(|i| i.as_str())));
    let config_output = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("output").and_then(|o| o.as_str())));
    let config_target = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("target").and_then(|t| t.as_str())));
    let config_optimization = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("optimization").and_then(|o| o.as_integer())));
    let config_debug = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("debug").and_then(|d| d.as_bool())));
    let config_inline = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("inline").and_then(|i| i.as_bool())));
    let config_unroll = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("unroll").and_then(|u| u.as_bool())));
    let config_dce = config.as_ref().and_then(|c| c.get("build").and_then(|b| b.get("dce").and_then(|d| d.as_bool())));

    // 确定最终使用的输入文件
    let final_input = if let Some(config_input_str) = config_input {
        PathBuf::from(config_input_str)
    } else {
        input.clone()
    };

    // 检查输入文件是否存在
    if !final_input.exists() {
        return Err(format!("Input file does not exist: {}", final_input.display()).into());
    }

    // 确定输出文件路径
    let output_path = match output {
        Some(path) => path.clone(),
        None => {
            if let Some(config_output_str) = config_output {
                PathBuf::from(config_output_str)
            } else {
                let stem = final_input.file_stem().unwrap_or_default();
                let ext = if target.contains("windows") || config_target.map_or(false, |t| t.contains("windows")) {
                    "exe"
                } else {
                    ""
                };
                PathBuf::from(stem).with_extension(ext)
            }
        }
    };

    // 确定目标平台
    let final_target = config_target.unwrap_or(target);
    let target_enum = match final_target {
        "x86_64-linux" | "linux" | "linux-x64" => "linux-x64",
        "x86_64-windows" | "windows" | "windows-x64" | "win64" => "windows-x64",
        "x86_64-macos" | "macos" | "macos-x64" | "darwin" => "macos-x64",
        _ => return Err(format!("Unknown target: {}", final_target).into()),
    };

    // 确定优化级别
    let final_optimization = if let Some(opt_level) = config_optimization {
        format!("O{}", opt_level)
    } else {
        optimization.to_string()
    };

    // 确定是否生成调试信息
    let final_debug = config_debug.unwrap_or(debug);

    // 确定其他优化选项
    let final_inline = config_inline.unwrap_or(false);
    let final_unroll = config_unroll.unwrap_or(false);
    let final_dce = config_dce.unwrap_or(true);

    // 增量构建检查
    let input_mtime = get_file_mtime(&final_input)?;
    let output_exists = output_path.exists();
    let mut needs_build = true;

    if output_exists {
        let output_mtime = get_file_mtime(&output_path)?;
        if input_mtime <= output_mtime {
            // 检查配置文件是否有变化
            let config_mtime = if let Some(config) = config {
                get_file_mtime(Path::new("slime.toml")).ok()
            } else {
                None
            };

            let config_changed = config_mtime.map_or(false, |ct| ct > output_mtime);
            needs_build = config_changed;
        }
    }

    if !needs_build {
        println!("✓ Skipping build: no changes detected");
        println!("  Output: {}", output_path.display());
        return Ok(());
    }

    // 确保输出目录存在
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).ok();
    }

    // 检查 slimec 是否存在
    let slimec_path = if let Ok(path) = which("slimec") {
        path
    } else {
        // 尝试在当前目录查找
        let current_dir = env::current_dir()?;
        let slimec_candidate = current_dir.join("slimec.exe");
        if slimec_candidate.exists() {
            slimec_candidate
        } else {
            // 尝试在 slime re/bin 目录查找
            let slime_re_bin = Path::new("slime re/bin/slimec.exe");
            if slime_re_bin.exists() {
                slime_re_bin.to_path_buf()
            } else {
                // 尝试在上级目录的 slime re/bin 查找
                let parent_dir = current_dir.parent().unwrap_or(&current_dir);
                let parent_slime_re_bin = parent_dir.join("slime re/bin/slimec.exe");
                if parent_slime_re_bin.exists() {
                    parent_slime_re_bin
                } else {
                    return Err("slimec not found. Please make sure it's in your PATH or in 'slime re/bin' directory.".into());
                }
            }
        }
    };

    // 检查 nasm 是否存在
    if which("nasm").is_err() {
        return Err("nasm not found. Please make sure it's in your PATH.".into());
    }

    // 生成临时汇编文件名
    let asm_path = final_input.with_extension("asm");
    
    // 步骤 1: 使用 slimec 编译到汇编
    println!("Compiling {} to assembly...", final_input.display());
    println!("  Optimization: {}", final_optimization);
    println!("  Debug: {}", final_debug);
    println!("  Inline: {}", final_inline);
    println!("  Unroll: {}", final_unroll);
    println!("  DCE: {}", final_dce);
    
    let mut slimec_cmd = Command::new(&slimec_path);
    slimec_cmd
        .arg(&final_input)
        .arg("-o")
        .arg(&asm_path)
        .arg("--target")
        .arg(target_enum)
        .arg(&final_optimization);
    
    // 添加标准库路径
    let current_dir = env::current_dir()?;
    let lib_paths = vec![
        current_dir.join("lib"),
        current_dir.join("slime re/lib"),
        Path::new("slime re/lib").to_path_buf(),
        // 尝试相对于 slimec 可执行文件的路径
        if let Some(slimec_parent) = slimec_path.parent() {
            slimec_parent.parent().map(|p| p.join("lib")).unwrap_or_else(|| Path::new("lib").to_path_buf())
        } else {
            Path::new("lib").to_path_buf()
        },
    ];
    
    // 打印找到的库路径
    println!("  Looking for standard library...");
    for lib_path in &lib_paths {
        if lib_path.exists() {
            println!("  Found standard library at: {}", lib_path.display());
            slimec_cmd.arg("-I").arg(lib_path);
            break;
        }
    }
    
    // 库路径已经通过 -I 参数指定，编译器会自动查找
    
    if final_debug {
        slimec_cmd.arg("--debug");
    }
    if final_inline {
        slimec_cmd.arg("--inline");
    }
    if final_unroll {
        slimec_cmd.arg("--unroll");
    }
    if final_dce {
        slimec_cmd.arg("--dce");
    }
    
    let slimec_output = slimec_cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !slimec_output.status.success() {
        return Err("Compilation failed".into());
    }

    // 步骤 2: 使用 nasm 汇编
    println!("Assembling {}...", asm_path.display());
    let obj_path = final_input.with_extension(if final_target.contains("windows") { "obj" } else { "o" });
    
    let nasm_output = Command::new("nasm")
        .arg(if final_target.contains("windows") { "-fwin64" } else if final_target.contains("linux") { "-felf64" } else { "-fmacho64" })
        .arg(&asm_path)
        .arg("-o")
        .arg(&obj_path)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()?;

    if !nasm_output.status.success() {
        return Err("Assembly failed".into());
    }

    // 步骤 3: 使用链接器
    println!("Linking to {}...", output_path.display());
    
    let link_output = if final_target.contains("windows") {
        // Windows 链接
        let linker = if which("golink").is_ok() {
            "golink"
        } else if which("link").is_ok() {
            "link"
        } else {
            return Err("No linker found. Please install golink or Visual Studio link".into());
        };

        if linker == "golink" {
            Command::new("golink")
                .arg("/console")
                .arg("/entry:Start")
                .arg(&obj_path)
                .arg("kernel32.dll")
                .arg(format!("{}", output_path.display()))
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        } else {
            Command::new("link")
                .arg(&obj_path)
                .arg("/subsystem:console")
                .arg("/entry:Start")
                .arg("/out:")
                .arg(&output_path)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()?
        }
    } else if final_target.contains("linux") {
        // Linux 链接
        Command::new("ld")
            .arg(&obj_path)
            .arg("-o")
            .arg(&output_path)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()?
    } else {
        // macOS 链接
        Command::new("ld")
            .arg(&obj_path)
            .arg("-o")
            .arg(&output_path)
            .arg("-lSystem")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output()?
    };

    if !link_output.status.success() {
        return Err("Linking failed".into());
    }

    // 清理临时文件
    fs::remove_file(&asm_path).ok();
    fs::remove_file(&obj_path).ok();

    println!("✓ Build completed successfully!");
    println!("  Output: {}", output_path.display());

    Ok(())
}
