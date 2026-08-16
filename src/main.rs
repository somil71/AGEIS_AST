use clap::Parser;
use needle::Result;

mod cli;

#[derive(Parser)]
#[command(name = "needle")]
#[command(version = "0.1.0")]
#[command(about = "Local-first hybrid search engine — keyword + semantic, offline, instant")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Initialize and build the search index for one or more directories
    Init {
        /// Directories to index
        #[arg(required = true)]
        directories: Vec<String>,
    },

    /// Search across indexed content (keyword + semantic hybrid)
    Search {
        /// Search query (exact term, identifier, or natural language description)
        query: String,

        /// Maximum number of results to return
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Show all results above threshold
        #[arg(long)]
        all: bool,

        /// Disable colored output
        #[arg(long)]
        no_color: bool,

        /// Compact output (one line per result, no snippet)
        #[arg(long)]
        compact: bool,

        /// Filter results to a specific language (rs, py, ts, js, go, md, …)
        #[arg(short = 'L', long)]
        lang: Option<String>,
    },

    /// Show index health and statistics
    Status,

    /// Force full re-index of all watched directories
    Reindex,

    /// View or edit configuration
    Config {
        #[command(subcommand)]
        action: Option<cli::config::ConfigAction>,
    },

    /// Run performance benchmarks (HNSW recall, query latency, index size)
    Bench,

    /// Watch indexed directories and re-index on file changes
    Watch,

    /// Start an MCP server over stdio for AI agent integration
    ///
    /// Add to agent config:
    ///   { "mcpServers": { "needle": { "command": "needle", "args": ["mcp"] } } }
    Mcp,

    /// Launch the web search UI in your browser
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "7700")]
        port: u16,

        /// Don't open the browser automatically
        #[arg(long)]
        no_open: bool,
    },

    /// Generate a Markdown report: god nodes, communities, and surprise edges
    Report {
        /// Output file path (default: GRAPH_REPORT.md)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Export an interactive D3 knowledge graph visualization to graph.html
    Graph {
        /// Output file path (default: graph.html)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Manage security and compliance policies (ingest, list)
    Policy {
        #[command(subcommand)]
        action: cli::policy::PolicyCommands,
    },

    /// Run diagnostic checks for system readiness and sovereign mode compliance
    Doctor {
        /// Verify sovereign mode (zero cloud routes, loopback LLM, ledger integrity)
        #[arg(long)]
        sovereign: bool,

        /// Strict offline validation (reject any non-loopback endpoint)
        #[arg(long)]
        offline_strict: bool,

        /// Ollama endpoint to probe (default: http://127.0.0.1:11434)
        #[arg(long, default_value = "http://127.0.0.1:11434")]
        ollama_url: String,

        /// Custom path to ledger audit chain file
        #[arg(long)]
        ledger_path: Option<String>,

        /// Output diagnostics as JSON
        #[arg(long)]
        json: bool,
    },

    /// Cryptographic audit ledger commands
    Ledger {
        #[command(subcommand)]
        action: cli::ledger::LedgerCommands,
    },

    /// Run policy-code compliance audit and generate a report
    Audit {
        /// Auto-append the report to the cryptographic audit ledger
        #[arg(long)]
        ledger: bool,

        /// Write report to file instead of stdout
        #[arg(short, long)]
        output: Option<String>,

        /// Output as JSON instead of Markdown
        #[arg(long)]
        json: bool,

        /// Return exit code 1 if there are any compliance violations (for CI/CD)
        #[arg(long)]
        strict: bool,

        /// Generate a PDF report via Edge headless
        #[arg(long)]
        pdf: Option<String>,
    },

    /// Automatically fix a security vulnerability using AI
    Fix {
        /// Path to the file containing the vulnerability
        #[arg(required = true)]
        file: String,

        /// Description or ID of the issue to fix
        #[arg(long)]
        issue: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "needle=warn".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Serve { port: 7700, no_open: false }) {
        Commands::Init { directories } => cli::init::run(directories).await?,
        Commands::Search { query, limit, all, no_color, compact, lang } => {
            cli::search::run(query, limit, all, no_color, compact, lang).await?
        }
        Commands::Status => cli::status::run().await?,
        Commands::Reindex => cli::reindex::run().await?,
        Commands::Config { action } => cli::config::run(action).await?,
        Commands::Bench => cli::bench::run().await?,
        Commands::Watch => cli::watch::run().await?,
        Commands::Mcp => cli::mcp::run().await?,
        Commands::Serve { port, no_open } => cli::serve::run(port, no_open).await?,
        Commands::Report { output } => cli::report::run(output).await?,
        Commands::Graph  { output } => cli::graph::run(output).await?,
        Commands::Policy { action } => cli::policy::run(action).await?,
        Commands::Doctor { sovereign, offline_strict, ollama_url, ledger_path, json } => {
            cli::doctor::run(sovereign, offline_strict, &ollama_url, ledger_path.as_deref(), json).await?;
        }
        Commands::Ledger { action } => cli::ledger::run(action).await?,
        Commands::Audit { ledger, output, json, strict, pdf } => cli::audit::run(ledger, output, json, strict, pdf).await?,
        Commands::Fix { file, issue } => cli::fix::run(&file, &issue).await?,
    }

    Ok(())
}
