use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand, ValueEnum, value_parser};
use hexaris_engine::{
    Architecture, BytePatch, BytePattern, Image, LoadMode, Project, analyze, analyze_function,
    decompile, disassemble, lift, parse_address, scan_strings,
};
use serde_json::json;

#[derive(Parser)]
#[command(version, about = "Local binary inspection and static analysis")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print executable metadata as JSON.
    Inspect(Input),
    /// Decode a sequential range of executable bytes as JSON.
    Disassemble {
        #[command(flatten)]
        input: Input,
        #[arg(value_parser = parse_address)]
        address: u64,
        #[arg(long, default_value = "128", value_parser = value_parser!(u32).range(1..=8192))]
        count: u32,
    },
    /// Recover a bounded C-like projection of a Windows x64 leaf function.
    Decompile {
        #[command(flatten)]
        input: Input,
        #[arg(value_parser = parse_address)]
        address: u64,
    },
    /// Print decoded instructions, blocks, and references as JSON.
    Analyze {
        #[command(flatten)]
        input: Input,
        #[arg(long, value_parser = parse_address)]
        address: Option<u64>,
        /// Include register IR for the selected function.
        #[arg(long, requires = "address")]
        lift: bool,
    },
    /// Print a range of file bytes.
    Hex {
        #[command(flatten)]
        input: Input,
        #[arg(long, default_value = "0", value_parser = parse_address)]
        offset: u64,
        #[arg(long, default_value = "256", value_parser = value_parser!(u32).range(1..=65536))]
        length: u32,
    },
    /// Find a hexadecimal byte pattern (?? matches any one byte).
    Find {
        #[command(flatten)]
        input: Input,
        pattern: String,
        #[arg(long, default_value = "100", value_parser = value_parser!(u32).range(1..=10000))]
        limit: u32,
    },
    /// Export a copy with an exact, fixed-length byte replacement.
    Patch {
        #[command(flatten)]
        input: Input,
        output: PathBuf,
        #[arg(value_parser = parse_address)]
        offset: u64,
        #[arg(long)]
        expect: String,
        #[arg(long)]
        replace: String,
    },
    /// Find printable ASCII runs.
    Strings {
        #[command(flatten)]
        input: Input,
        #[arg(long, default_value = "5", value_parser = value_parser!(u32).range(1..))]
        min_length: u32,
        #[arg(long, default_value = "100", value_parser = value_parser!(u32).range(1..=10000))]
        limit: u32,
    },
    /// Create and edit local analysis projects.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
}

#[derive(Args)]
struct Input {
    /// Binary or .hxproj file.
    path: PathBuf,
    #[arg(long, value_enum)]
    raw: Option<RawArchitecture>,
    #[arg(long, requires = "raw", value_parser = parse_address)]
    base: Option<u64>,
}

#[derive(Clone, Copy, ValueEnum)]
enum RawArchitecture {
    X86,
    X86_64,
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Create a portable project that embeds the input binary.
    Create {
        #[command(flatten)]
        input: Input,
        output: PathBuf,
    },
    /// Update an address comment or function name. An empty value removes it.
    Annotate {
        path: PathBuf,
        #[arg(value_parser = parse_address)]
        address: u64,
        #[arg(long, required_unless_present = "name")]
        comment: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Read annotations for an address.
    Annotation {
        path: PathBuf,
        #[arg(value_parser = parse_address)]
        address: u64,
    },
}

impl Input {
    fn open(&self) -> hexaris_engine::Result<Project> {
        if self.raw.is_none() && is_project(&self.path) {
            return Project::open(&self.path);
        }
        let mode = match self.raw {
            Some(architecture) => LoadMode::Raw {
                architecture: match architecture {
                    RawArchitecture::X86 => Architecture::X86,
                    RawArchitecture::X86_64 => Architecture::X86_64,
                },
                base: self.base.unwrap_or(0),
            },
            None => LoadMode::Auto,
        };
        let image = Image::open(&self.path, mode)?;
        let name = self
            .path
            .file_name()
            .unwrap_or(self.path.as_os_str())
            .to_string_lossy()
            .into_owned();
        Ok(Project::new(name, image))
    }
}

fn is_project(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("hxproj"))
}

fn write_json(
    output: &mut impl Write,
    value: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer_pretty(&mut *output, value)?;
    writeln!(output)?;
    Ok(())
}

fn execute(command: Command, output: &mut impl Write) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Inspect(input) => write_json(output, &json!(input.open()?.image().info))?,
        Command::Disassemble {
            input,
            address,
            count,
        } => {
            let project = input.open()?;
            write_json(
                output,
                &json!(disassemble(project.image(), address, count as usize)?),
            )?;
        }
        Command::Decompile { input, address } => {
            let project = input.open()?;
            let function = analyze_function(project.image(), address)?;
            write_json(output, &json!(decompile(project.image(), &function)))?;
        }
        Command::Analyze {
            input,
            address,
            lift: with_lift,
        } => {
            let project = input.open()?;
            if let Some(address) = address {
                let mut function = analyze_function(project.image(), address)?;
                if let Some(name) = project.function_name(address) {
                    function.name = name.into();
                }
                if with_lift {
                    write_json(
                        output,
                        &json!({ "ir": lift(&function), "function": function }),
                    )?;
                } else {
                    write_json(output, &json!(function))?;
                }
            } else {
                let mut analysis = analyze(project.image());
                for (&address, function) in &mut analysis.functions {
                    if let Some(name) = project.function_name(address) {
                        function.name = name.into();
                    }
                }
                write_json(output, &json!(analysis))?;
            }
        }
        Command::Hex {
            input,
            offset,
            length,
        } => {
            let project = input.open()?;
            let image = project.image();
            let offset = usize::try_from(offset)?;
            let bytes = image
                .bytes()
                .get(offset..)
                .filter(|bytes| !bytes.is_empty())
                .ok_or("offset is outside the binary")?;
            for (index, chunk) in bytes[..bytes.len().min(length as usize)]
                .chunks(16)
                .enumerate()
            {
                write!(output, "{:08X}  ", offset + index * 16)?;
                for byte in chunk {
                    write!(output, "{byte:02X} ")?;
                }
                writeln!(output)?;
            }
        }
        Command::Find {
            input,
            pattern,
            limit,
        } => {
            let project = input.open()?;
            let image = project.image();
            let pattern = BytePattern::parse(&pattern)?;
            let hits: Vec<_> = pattern
                .find(image.bytes())
                .take(limit as usize)
                .map(|offset| json!({ "offset": offset, "address": image.virtual_address(offset) }))
                .collect();
            write_json(output, &json!(hits))?;
        }
        Command::Strings {
            input,
            min_length,
            limit,
        } => {
            let project = input.open()?;
            let items: Vec<_> = scan_strings(project.image().bytes(), min_length as usize)
                .take(limit as usize)
                .collect();
            write_json(output, &json!(items))?;
        }
        Command::Patch {
            input,
            output,
            offset,
            expect,
            replace,
        } => {
            let project = input.open()?;
            let patch = BytePatch::parse(usize::try_from(offset)?, &expect, &replace)?;
            patch.write_copy(project.image(), &output)?;
        }
        Command::Project { action } => match action {
            ProjectAction::Create { input, output } => {
                if !is_project(&output) {
                    return Err("output must end in .hxproj".into());
                }
                if output.exists() {
                    return Err("project output already exists".into());
                }
                input.open()?.save_new(&output)?;
            }
            ProjectAction::Annotate {
                path,
                address,
                comment,
                name,
            } => {
                let mut project = Project::open(&path)?;
                if let Some(value) = comment {
                    project.set_comment(address, value)?;
                }
                if let Some(value) = name {
                    project.set_function_name(address, value)?;
                }
                project.save(&path)?;
            }
            ProjectAction::Annotation { path, address } => {
                let project = Project::open(&path)?;
                write_json(
                    output,
                    &json!({ "address": address, "comment": project.comment(address), "name": project.function_name(address) }),
                )?;
            }
        },
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    let mut output = io::BufWriter::new(io::stdout().lock());
    if let Err(error) =
        execute(Cli::parse().command, &mut output).and_then(|()| Ok(output.flush()?))
    {
        if error
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
        {
            return std::process::ExitCode::SUCCESS;
        }
        eprintln!("hexaris-cli: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
