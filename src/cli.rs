#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildTarget {
    Native,
    Web,
    Llvm,
}

impl BuildTarget {
    pub fn from_cli(value: &str) -> Option<Self> {
        match value {
            "native" => Some(Self::Native),
            "web" | "wasm" => Some(Self::Web),
            "llvm" | "ir" => Some(Self::Llvm),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BuildTarget::Native => "native",
            BuildTarget::Web => "web",
            BuildTarget::Llvm => "llvm",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildProfile {
    Dev,
    Fast,
}

impl BuildProfile {
    pub fn from_cli(value: &str) -> Option<Self> {
        match value {
            "dev" => Some(Self::Dev),
            "fast" => Some(Self::Fast),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            BuildProfile::Dev => "dev",
            BuildProfile::Fast => "fast",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CliOptions {
    pub input: String,
    pub output: Option<String>,
    pub target: BuildTarget,
    pub profile: BuildProfile,
    pub emit_rust: bool,
}

pub fn parse_cli(args: &[String]) -> Result<CliOptions, String> {
    let mut input: Option<String> = None;
    let mut output: Option<String> = None;
    let mut target = BuildTarget::Native;
    let mut profile = BuildProfile::Dev;
    let mut emit_rust = true;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("faltou valor para --target".to_string());
                };
                target = BuildTarget::from_cli(value)
                    .ok_or_else(|| format!("target inválido: '{value}' (use native, web ou llvm)"))?;
            }
            "--profile" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("faltou valor para --profile".to_string());
                };
                profile = BuildProfile::from_cli(value)
                    .ok_or_else(|| format!("profile inválido: '{value}' (use dev ou fast)"))?;
            }
            "--output" | "-o" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err("faltou valor para --output".to_string());
                };
                output = Some(value.clone());
            }
            "--web" => {
                target = BuildTarget::Web;
            }
            "--llvm" => {
                target = BuildTarget::Llvm;
            }
            "--fast" => {
                profile = BuildProfile::Fast;
            }
            "--no-emit-rust" => {
                emit_rust = false;
            }
            "--help" | "-h" => {}
            option if option.starts_with('-') => {
                return Err(format!("flag desconhecida: '{option}'"));
            }
            path => {
                if input.is_some() {
                    return Err(format!(
                        "apenas um arquivo de entrada é suportado, valor extra: '{path}'"
                    ));
                }
                input = Some(path.to_string());
            }
        }
        i += 1;
    }

    let input = input.ok_or_else(|| "faltou informar o arquivo .ikn".to_string())?;
    Ok(CliOptions {
        input,
        output,
        target,
        profile,
        emit_rust,
    })
}

pub fn print_help(bin: &str) {
    println!("Likn Lang Compiler");
    println!();
    println!("Uso:");
    println!("  {bin} [FLAGS] <arquivo.ikn>");
    println!();
    println!("Flags:");
    println!("  --help, -h            Exibe esta ajuda");
    println!("  --target <native|web|llvm> Define o alvo de build");
    println!("  --web                 Atalho para --target web");
    println!("  --llvm                Atalho para --target llvm");
    println!("  --profile <dev|fast>  Define o perfil de otimização");
    println!("  --fast                Atalho para --profile fast");
    println!("  --output, -o <path>   Define o arquivo de saída");
    println!("  --no-emit-rust        Não mantém o .rs gerado");
}
