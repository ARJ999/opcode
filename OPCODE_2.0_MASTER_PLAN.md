# OPCODE 2.0 - The World's Greatest Claude Code Wrapper
## Master Development Plan

**Version**: 2.0.0
**Codename**: "Apex"
**Target**: Transform Opcode into the definitive Claude Code experience
**Powered by**: Claude Opus 4.5

---

## Executive Summary

This master plan outlines the comprehensive upgrade of Opcode from a capable wrapper to the **world's greatest Claude Code interface**. Based on extensive research of:

- Latest Claude Code CLI features (v2.1.0+)
- MCP Specification 2025-11-25 (Streamable HTTP, OAuth 2.1, Async Tasks)
- Current Opcode limitations (45+ identified issues)
- Enterprise deployment requirements

---

## Table of Contents

1. [Phase 1: Critical Infrastructure Overhaul](#phase-1-critical-infrastructure-overhaul)
2. [Phase 2: MCP Streamable HTTP & Remote Server Support](#phase-2-mcp-streamable-http--remote-server-support)
3. [Phase 3: Skills & Extensions Ecosystem](#phase-3-skills--extensions-ecosystem)
4. [Phase 4: Latest Claude Code Features](#phase-4-latest-claude-code-features)
5. [Phase 5: Enterprise & Production Features](#phase-5-enterprise--production-features)
6. [Phase 6: Advanced UX & Intelligence](#phase-6-advanced-ux--intelligence)
7. [Architecture Diagrams](#architecture-diagrams)
8. [Implementation Priority Matrix](#implementation-priority-matrix)

---

## Phase 1: Critical Infrastructure Overhaul

### 1.1 Fix Web Server Session Isolation (CRITICAL)

**Current Problem**: All sessions receive all events; no session-specific routing.

**Solution**:
```rust
// New session-aware event system
pub struct SessionManager {
    sessions: Arc<DashMap<String, SessionState>>,
    process_handles: Arc<DashMap<String, ProcessHandle>>,
}

pub struct SessionState {
    id: String,
    websocket_tx: broadcast::Sender<SessionEvent>,
    process_handle: Option<Child>,
    created_at: DateTime<Utc>,
    status: SessionStatus,
}
```

**Implementation**:
- [ ] Create `SessionManager` with DashMap for concurrent access
- [ ] Route events to specific session WebSocket channels
- [ ] Implement session lifecycle management (create, suspend, resume, terminate)
- [ ] Add session heartbeat and timeout handling

### 1.2 Process Management & Cancellation (CRITICAL)

**Current Problem**: Cancel endpoint is a stub; processes can't be terminated.

**Solution**:
```rust
pub struct ProcessRegistry {
    processes: Arc<DashMap<String, ManagedProcess>>,
}

pub struct ManagedProcess {
    session_id: String,
    child: Child,
    pid: u32,
    stdout_handle: JoinHandle<()>,
    stderr_handle: JoinHandle<()>,
    kill_switch: oneshot::Sender<()>,
}

impl ProcessRegistry {
    pub async fn kill_process(&self, session_id: &str) -> Result<()> {
        if let Some((_, mut process)) = self.processes.remove(session_id) {
            // Send kill signal
            let _ = process.kill_switch.send(());
            // Force kill if graceful fails
            process.child.kill().await?;
            // Cleanup handles
            process.stdout_handle.abort();
            process.stderr_handle.abort();
        }
        Ok(())
    }
}
```

**Implementation**:
- [ ] Implement proper process handle storage
- [ ] Add graceful shutdown with timeout
- [ ] Implement force kill fallback
- [ ] Add orphan process detection and cleanup
- [ ] Emit proper `claude-cancelled` events

### 1.3 Complete stderr Capture (HIGH)

**Current Problem**: Only stdout captured; errors invisible.

**Solution**:
```rust
async fn spawn_claude_with_full_output(
    cmd: Command,
    session_id: String,
    event_tx: broadcast::Sender<SessionEvent>,
) -> Result<ManagedProcess> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Spawn stdout reader
    let stdout_tx = event_tx.clone();
    let stdout_handle = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stdout_tx.send(SessionEvent::Output(line));
        }
    });

    // Spawn stderr reader
    let stderr_tx = event_tx.clone();
    let stderr_handle = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stderr_tx.send(SessionEvent::Error(line));
        }
    });

    // ...
}
```

### 1.4 Database Connection Pooling (HIGH)

**Current Problem**: Mutex-based locking blocks async operations.

**Solution**:
```rust
use deadpool_sqlite::{Config, Pool, Runtime};

pub struct DatabasePool {
    pool: Pool,
}

impl DatabasePool {
    pub async fn new(db_path: &Path) -> Result<Self> {
        let config = Config::new(db_path);
        let pool = config.create_pool(Runtime::Tokio1)?;
        Ok(Self { pool })
    }

    pub async fn execute<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = self.pool.get().await?;
        tokio::task::spawn_blocking(move || f(&conn)).await?
    }
}
```

### 1.5 Concurrent Session Support (HIGH)

**Current Problem**: Only one Claude process at a time in Tauri mode.

**Solution**:
- Replace single `ClaudeProcessState` with `ProcessRegistry`
- Support unlimited concurrent sessions
- Implement resource limits (configurable max sessions)
- Add session priority queue for resource management

---

## Phase 2: MCP Streamable HTTP & Remote Server Support

### 2.1 Streamable HTTP Transport Implementation

**Current Problem**: Only supports STDIO and legacy SSE transports.

**New MCP Transport Architecture**:
```rust
pub enum McpTransport {
    Stdio(StdioTransport),
    StreamableHttp(StreamableHttpTransport),
    #[deprecated]
    Sse(SseTransport), // Keep for backward compatibility
}

pub struct StreamableHttpTransport {
    endpoint: Url,
    client: reqwest::Client,
    session_id: Option<String>,
    auth: Option<McpAuth>,
}

impl StreamableHttpTransport {
    pub async fn connect(&mut self) -> Result<()> {
        // Initial handshake
        let response = self.client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {
                        "tools": {},
                        "resources": {},
                        "prompts": {},
                        "sampling": {}
                    },
                    "clientInfo": {
                        "name": "opcode",
                        "version": "2.0.0"
                    }
                },
                "id": 1
            }))
            .send()
            .await?;

        // Extract session ID from header
        if let Some(session_id) = response.headers().get("Mcp-Session-Id") {
            self.session_id = Some(session_id.to_str()?.to_string());
        }

        Ok(())
    }

    pub async fn call_tool(&self, name: &str, args: Value) -> Result<Value> {
        let mut request = self.client
            .post(&self.endpoint)
            .header("Content-Type", "application/json");

        if let Some(ref session_id) = self.session_id {
            request = request.header("Mcp-Session-Id", session_id);
        }

        if let Some(ref auth) = self.auth {
            request = auth.apply(request);
        }

        let response = request
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "tools/call",
                "params": {
                    "name": name,
                    "arguments": args
                },
                "id": uuid::Uuid::new_v4().to_string()
            }))
            .send()
            .await?;

        // Handle streaming response if SSE
        if response.headers().get("content-type")
            .map(|v| v.to_str().unwrap_or(""))
            .unwrap_or("")
            .contains("text/event-stream")
        {
            self.handle_sse_stream(response).await
        } else {
            response.json().await
        }
    }
}
```

### 2.2 Remote MCP Server Management UI

**New Remote Server Configuration**:
```typescript
interface RemoteMcpServer {
  id: string;
  name: string;
  description?: string;
  endpoint: string;                    // https://mcp.your-vps.com/server
  transport: 'streamable-http';        // Only streamable-http for remote
  auth: McpServerAuth;
  healthCheck: {
    enabled: boolean;
    interval: number;                  // seconds
    timeout: number;                   // seconds
  };
  capabilities?: McpCapabilities;      // Auto-discovered
  status: 'connected' | 'disconnected' | 'error';
  lastHealthCheck?: Date;
  metadata?: Record<string, unknown>;
}

interface McpServerAuth {
  type: 'none' | 'bearer' | 'oauth2' | 'api-key' | 'custom-header';
  // Bearer token
  token?: string;
  // OAuth 2.1
  oauth?: {
    authorizationEndpoint: string;
    tokenEndpoint: string;
    clientId: string;
    clientSecret?: string;             // For confidential clients
    scopes: string[];
    pkce: boolean;                     // Always true for public clients
  };
  // API Key
  apiKey?: {
    header: string;                    // e.g., "X-API-Key"
    value: string;
  };
  // Custom headers
  customHeaders?: Record<string, string>;
}
```

**Remote Server Manager Component**:
```typescript
// src/components/RemoteMcpManager.tsx
export function RemoteMcpManager() {
  return (
    <div className="remote-mcp-manager">
      {/* Server List */}
      <ServerList
        servers={servers}
        onSelect={setSelectedServer}
        onHealthCheck={checkServerHealth}
      />

      {/* Add New Server */}
      <AddRemoteServer
        onAdd={handleAddServer}
        onTest={testConnection}
      />

      {/* Server Details Panel */}
      <ServerDetailsPanel
        server={selectedServer}
        onEdit={handleEditServer}
        onDelete={handleDeleteServer}
      >
        {/* Capabilities Explorer */}
        <CapabilitiesExplorer capabilities={serverCapabilities} />

        {/* Tool Tester */}
        <ToolTester
          tools={serverTools}
          onInvoke={invokeToolTest}
        />

        {/* Health & Metrics */}
        <ServerHealthMetrics
          metrics={serverMetrics}
          history={healthHistory}
        />
      </ServerDetailsPanel>
    </div>
  );
}
```

### 2.3 MCP Server Health Monitoring

```rust
pub struct McpHealthMonitor {
    servers: Arc<DashMap<String, McpServerHealth>>,
    check_interval: Duration,
}

pub struct McpServerHealth {
    server_id: String,
    status: HealthStatus,
    latency_ms: Option<u64>,
    last_check: DateTime<Utc>,
    consecutive_failures: u32,
    capabilities_hash: Option<String>,
}

impl McpHealthMonitor {
    pub async fn start_monitoring(&self) {
        loop {
            for entry in self.servers.iter() {
                let server = entry.value();
                self.check_server_health(&server.server_id).await;
            }
            tokio::time::sleep(self.check_interval).await;
        }
    }

    async fn check_server_health(&self, server_id: &str) -> Result<HealthStatus> {
        let start = Instant::now();

        // Send ping/health check
        let response = self.client
            .post(&format!("{}/health", server.endpoint))
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        let latency = start.elapsed().as_millis() as u64;

        match response {
            Ok(resp) if resp.status().is_success() => {
                self.update_health(server_id, HealthStatus::Healthy, Some(latency));
                Ok(HealthStatus::Healthy)
            }
            Ok(resp) => {
                self.update_health(server_id, HealthStatus::Degraded, Some(latency));
                Ok(HealthStatus::Degraded)
            }
            Err(_) => {
                self.update_health(server_id, HealthStatus::Unhealthy, None);
                Ok(HealthStatus::Unhealthy)
            }
        }
    }
}
```

### 2.4 OAuth 2.1 Authentication for MCP

```rust
pub struct McpOAuthManager {
    token_cache: Arc<DashMap<String, OAuthTokenSet>>,
    pkce_verifiers: Arc<DashMap<String, String>>,
}

pub struct OAuthTokenSet {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: DateTime<Utc>,
    scopes: Vec<String>,
}

impl McpOAuthManager {
    pub async fn authenticate(&self, server: &RemoteMcpServer) -> Result<String> {
        // Check cache first
        if let Some(tokens) = self.token_cache.get(&server.id) {
            if tokens.expires_at > Utc::now() + Duration::minutes(5) {
                return Ok(tokens.access_token.clone());
            }
            // Try refresh
            if let Some(ref refresh_token) = tokens.refresh_token {
                if let Ok(new_tokens) = self.refresh_token(server, refresh_token).await {
                    self.token_cache.insert(server.id.clone(), new_tokens.clone());
                    return Ok(new_tokens.access_token);
                }
            }
        }

        // Full OAuth flow with PKCE
        self.start_oauth_flow(server).await
    }

    async fn start_oauth_flow(&self, server: &RemoteMcpServer) -> Result<String> {
        let oauth = server.auth.oauth.as_ref().unwrap();

        // Generate PKCE challenge
        let verifier = generate_pkce_verifier();
        let challenge = generate_pkce_challenge(&verifier);

        self.pkce_verifiers.insert(server.id.clone(), verifier);

        // Build authorization URL
        let auth_url = format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&state={}",
            oauth.authorization_endpoint,
            oauth.client_id,
            encode("opcode://oauth/callback"),
            oauth.scopes.join(" "),
            challenge,
            server.id
        );

        // Open browser for user authorization
        open::that(&auth_url)?;

        // Wait for callback (handled by Tauri deep link)
        self.wait_for_callback(&server.id).await
    }
}
```

---

## Phase 3: Skills & Extensions Ecosystem

### 3.1 Unified Skills Manager

**Concept**: One place to create, manage, import, and share all Claude Code extensions.

```typescript
interface Skill {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  type: SkillType;
  source: SkillSource;
  config: SkillConfig;
  enabled: boolean;
  installedAt: Date;
}

type SkillType =
  | 'slash-command'      // Custom slash commands
  | 'agent'              // Custom agents with system prompts
  | 'mcp-server'         // MCP server integration
  | 'hook'               // Pre/post tool hooks
  | 'plugin'             // Full plugin packages
  | 'template'           // Project templates
  | 'workflow';          // Multi-step workflows

interface SkillSource {
  type: 'local' | 'github' | 'registry' | 'url';
  path?: string;         // For local
  repo?: string;         // For github (e.g., "user/repo")
  registryId?: string;   // For registry
  url?: string;          // For direct URL
}
```

**Skills Manager UI**:
```typescript
// src/components/SkillsManager.tsx
export function SkillsManager() {
  return (
    <div className="skills-manager">
      {/* Skill Categories */}
      <SkillCategoryTabs
        categories={['All', 'Slash Commands', 'Agents', 'MCP Servers', 'Hooks', 'Workflows']}
        selected={selectedCategory}
        onChange={setSelectedCategory}
      />

      {/* Installed Skills */}
      <SkillGrid
        skills={filteredSkills}
        onToggle={toggleSkill}
        onConfigure={openSkillConfig}
        onDelete={deleteSkill}
      />

      {/* Quick Actions */}
      <QuickActions>
        <CreateSkillButton onClick={() => setShowCreator(true)} />
        <ImportFromGitHubButton onClick={() => setShowGitHubImport(true)} />
        <BrowseRegistryButton onClick={() => setShowRegistry(true)} />
      </QuickActions>

      {/* Skill Creator Modal */}
      <SkillCreatorWizard
        open={showCreator}
        onClose={() => setShowCreator(false)}
        onSave={saveNewSkill}
      />
    </div>
  );
}
```

### 3.2 Visual Skill Creator

**No-code/low-code skill creation**:

```typescript
// src/components/SkillCreatorWizard.tsx
export function SkillCreatorWizard({ onSave }: Props) {
  const [step, setStep] = useState(1);
  const [skillType, setSkillType] = useState<SkillType>();
  const [config, setConfig] = useState<SkillConfig>({});

  return (
    <Wizard currentStep={step}>
      {/* Step 1: Choose Type */}
      <WizardStep title="What do you want to create?">
        <SkillTypeSelector
          options={[
            { type: 'slash-command', icon: '/', title: 'Slash Command', description: 'Quick reusable prompts' },
            { type: 'agent', icon: '🤖', title: 'Custom Agent', description: 'Specialized AI with custom persona' },
            { type: 'mcp-server', icon: '🔌', title: 'MCP Server', description: 'Connect external tools' },
            { type: 'hook', icon: '🪝', title: 'Hook', description: 'Automate before/after actions' },
            { type: 'workflow', icon: '⚡', title: 'Workflow', description: 'Multi-step automation' },
          ]}
          selected={skillType}
          onSelect={setSkillType}
        />
      </WizardStep>

      {/* Step 2: Configure (Dynamic based on type) */}
      <WizardStep title="Configure your skill">
        {skillType === 'slash-command' && (
          <SlashCommandConfigurator
            config={config}
            onChange={setConfig}
          />
        )}
        {skillType === 'agent' && (
          <AgentConfigurator
            config={config}
            onChange={setConfig}
          />
        )}
        {skillType === 'mcp-server' && (
          <McpServerConfigurator
            config={config}
            onChange={setConfig}
          />
        )}
        {/* ... other configurators */}
      </WizardStep>

      {/* Step 3: Test */}
      <WizardStep title="Test your skill">
        <SkillTester
          skillType={skillType}
          config={config}
        />
      </WizardStep>

      {/* Step 4: Save & Deploy */}
      <WizardStep title="Save & Deploy">
        <DeployOptions
          options={['local', 'project', 'share']}
          onDeploy={(option) => onSave(config, option)}
        />
      </WizardStep>
    </Wizard>
  );
}
```

### 3.3 Slash Command Builder

```typescript
// Visual builder for slash commands
interface SlashCommandConfig {
  name: string;                    // Command name (e.g., "deploy")
  description: string;
  arguments: ArgumentDef[];
  prompt: string;                  // Main prompt template
  allowedTools: string[];          // Tool whitelist
  model?: string;                  // Override model
  context?: 'fork' | 'continue';   // Session context
  hooks?: {
    pre?: HookDef[];
    post?: HookDef[];
  };
  bashEmbeds?: BashEmbed[];        // Embedded bash commands
}

interface ArgumentDef {
  name: string;
  type: 'string' | 'number' | 'boolean' | 'file' | 'directory';
  required: boolean;
  default?: string;
  description: string;
}

interface BashEmbed {
  id: string;
  command: string;
  variable: string;                // Variable name to use in prompt
}
```

**Slash Command Builder UI**:
```typescript
export function SlashCommandConfigurator({ config, onChange }) {
  return (
    <div className="slash-command-builder">
      {/* Basic Info */}
      <Section title="Basic Info">
        <Input label="Command Name" prefix="/" value={config.name} />
        <Textarea label="Description" value={config.description} />
      </Section>

      {/* Arguments */}
      <Section title="Arguments">
        <ArgumentBuilder
          arguments={config.arguments}
          onChange={(args) => onChange({...config, arguments: args})}
        />
      </Section>

      {/* Prompt Editor */}
      <Section title="Prompt Template">
        <PromptEditor
          value={config.prompt}
          arguments={config.arguments}
          bashEmbeds={config.bashEmbeds}
          onChange={(prompt) => onChange({...config, prompt})}
        >
          {/* Variable inserter */}
          <VariableInserter
            variables={[
              { name: '$ARGUMENTS', description: 'All arguments' },
              { name: '$1, $2, ...', description: 'Positional args' },
              ...config.arguments.map(a => ({
                name: `$${a.name}`,
                description: a.description
              })),
            ]}
          />
          {/* Bash embed inserter */}
          <BashEmbedInserter
            onAdd={(embed) => onChange({
              ...config,
              bashEmbeds: [...config.bashEmbeds, embed]
            })}
          />
        </PromptEditor>
      </Section>

      {/* Tool Permissions */}
      <Section title="Allowed Tools">
        <ToolSelector
          selected={config.allowedTools}
          onChange={(tools) => onChange({...config, allowedTools: tools})}
        />
      </Section>

      {/* Advanced Options */}
      <Collapsible title="Advanced Options">
        <ModelSelector value={config.model} />
        <ContextModeSelector value={config.context} />
        <HooksBuilder hooks={config.hooks} />
      </Collapsible>
    </div>
  );
}
```

### 3.4 Workflow Builder (Visual DAG Editor)

**Multi-step workflow automation**:

```typescript
interface Workflow {
  id: string;
  name: string;
  description: string;
  trigger: WorkflowTrigger;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  variables: WorkflowVariable[];
}

interface WorkflowNode {
  id: string;
  type: 'prompt' | 'tool' | 'condition' | 'loop' | 'parallel' | 'mcp-call' | 'human-input';
  position: { x: number; y: number };
  config: NodeConfig;
}

interface WorkflowEdge {
  id: string;
  source: string;
  target: string;
  condition?: string;  // For conditional edges
}
```

**Visual Workflow Editor**:
```typescript
// Using React Flow for DAG visualization
import ReactFlow, { Controls, Background } from 'reactflow';

export function WorkflowEditor({ workflow, onChange }) {
  return (
    <div className="workflow-editor">
      <ReactFlow
        nodes={workflow.nodes}
        edges={workflow.edges}
        nodeTypes={customNodeTypes}
        onNodesChange={handleNodesChange}
        onEdgesChange={handleEdgesChange}
        onConnect={handleConnect}
      >
        <Controls />
        <Background />
      </ReactFlow>

      {/* Node Palette */}
      <NodePalette
        nodes={[
          { type: 'prompt', label: 'Claude Prompt', icon: '💬' },
          { type: 'tool', label: 'Tool Call', icon: '🔧' },
          { type: 'condition', label: 'Condition', icon: '❓' },
          { type: 'loop', label: 'Loop', icon: '🔄' },
          { type: 'parallel', label: 'Parallel', icon: '⚡' },
          { type: 'mcp-call', label: 'MCP Server', icon: '🔌' },
          { type: 'human-input', label: 'Human Input', icon: '👤' },
        ]}
        onDrag={handleNodeDrag}
      />

      {/* Node Config Panel */}
      <NodeConfigPanel
        selectedNode={selectedNode}
        onUpdate={handleNodeUpdate}
      />
    </div>
  );
}
```

---

## Phase 4: Latest Claude Code Features

### 4.1 Custom Agents System Enhancement

**Upgrade agent system to match Claude Code 2.1 capabilities**:

```typescript
interface EnhancedAgent {
  id: string;
  name: string;
  icon: string;
  description: string;

  // Core Configuration
  systemPrompt: string;
  appendSystemPrompt?: string;
  model: string;
  fallbackModel?: string;

  // Permissions (Enhanced)
  permissions: {
    mode: 'default' | 'plan' | 'acceptEdits' | 'bypassPermissions';
    allowRules: PermissionRule[];
    denyRules: PermissionRule[];
  };

  // Tool Configuration
  tools: {
    allowed: string[];              // Whitelist
    disallowed: string[];           // Blacklist
    mcpServers: string[];           // Enabled MCP servers
  };

  // Subagents (NEW - matching Claude 2.1)
  subagents?: SubagentDef[];

  // Hooks
  hooks: {
    preToolUse?: HookDef[];
    postToolUse?: HookDef[];
    onStart?: HookDef[];
    onComplete?: HookDef[];
    onError?: HookDef[];
  };

  // Advanced
  maxTurns?: number;
  contextWindow?: number;
  temperature?: number;
}

interface SubagentDef {
  name: string;
  description: string;
  systemPrompt: string;
  tools: string[];
  model?: string;
}
```

### 4.2 Enhanced Checkpoint System

**Upgrade checkpoints to match Claude Code 2.1**:

```rust
pub struct EnhancedCheckpointManager {
    storage: CheckpointStorage,
    file_watcher: FileWatcher,
    git_integration: Option<GitIntegration>,
}

impl EnhancedCheckpointManager {
    // Automatic checkpoint on significant changes
    pub async fn auto_checkpoint(&self, trigger: CheckpointTrigger) -> Result<Checkpoint> {
        match trigger {
            CheckpointTrigger::ToolUse(tool) => {
                if self.is_file_modifying_tool(&tool) {
                    self.create_checkpoint(Some(format!("After {}", tool.name))).await
                } else {
                    Ok(())
                }
            }
            CheckpointTrigger::Timer(interval) => {
                // Periodic checkpoints
                self.create_checkpoint(Some("Periodic checkpoint")).await
            }
            CheckpointTrigger::GitCommit(commit_id) => {
                // Checkpoint aligned with git
                self.create_checkpoint_with_git(&commit_id).await
            }
        }
    }

    // Instant rewind (Esc twice or /rewind)
    pub async fn quick_rewind(&self) -> Result<()> {
        let checkpoints = self.list_checkpoints().await?;
        if let Some(prev) = checkpoints.get(1) {  // Previous checkpoint
            self.restore_checkpoint(&prev.id).await?;
        }
        Ok(())
    }

    // Visual diff between checkpoints
    pub async fn generate_diff(&self, from: &str, to: &str) -> Result<CheckpointDiff> {
        let from_files = self.get_checkpoint_files(from).await?;
        let to_files = self.get_checkpoint_files(to).await?;

        // Generate unified diff
        let mut diffs = Vec::new();
        for (path, content) in to_files.iter() {
            if let Some(from_content) = from_files.get(path) {
                if from_content != content {
                    diffs.push(FileDiff {
                        path: path.clone(),
                        diff: generate_unified_diff(from_content, content),
                        additions: count_additions(&diff),
                        deletions: count_deletions(&diff),
                    });
                }
            } else {
                diffs.push(FileDiff {
                    path: path.clone(),
                    diff: format!("+++ {}\n{}", path, content),
                    additions: content.lines().count(),
                    deletions: 0,
                });
            }
        }

        Ok(CheckpointDiff { diffs })
    }
}
```

### 4.3 Parallel Task Execution

**Support for background tasks (Claude 2.1 feature)**:

```rust
pub struct TaskManager {
    foreground_task: Option<TaskHandle>,
    background_tasks: DashMap<String, TaskHandle>,
    task_queue: mpsc::Sender<Task>,
}

pub struct TaskHandle {
    id: String,
    name: String,
    status: TaskStatus,
    process: Child,
    output_buffer: Arc<Mutex<String>>,
    started_at: DateTime<Utc>,
}

impl TaskManager {
    pub async fn run_in_background(&self, task: Task) -> Result<String> {
        let task_id = uuid::Uuid::new_v4().to_string();

        // Spawn background task
        let handle = self.spawn_task(task).await?;
        self.background_tasks.insert(task_id.clone(), handle);

        // Notify UI
        self.emit_event(TaskEvent::Started {
            task_id: task_id.clone(),
            background: true
        });

        Ok(task_id)
    }

    pub async fn get_background_task_status(&self, task_id: &str) -> Option<TaskStatus> {
        self.background_tasks.get(task_id).map(|h| h.status.clone())
    }

    pub async fn bring_to_foreground(&self, task_id: &str) -> Result<()> {
        if let Some((_, handle)) = self.background_tasks.remove(task_id) {
            // Pause current foreground if exists
            if let Some(fg) = self.foreground_task.take() {
                self.background_tasks.insert(fg.id.clone(), fg);
            }
            self.foreground_task = Some(handle);
        }
        Ok(())
    }
}
```

### 4.4 Real-Time Thinking Display

**Ctrl+O transcript mode showing Claude's reasoning**:

```typescript
// src/components/ThinkingTranscript.tsx
export function ThinkingTranscript({ sessionId }: Props) {
  const [thinking, setThinking] = useState<ThinkingBlock[]>([]);
  const [showThinking, setShowThinking] = useState(false);

  // Listen for thinking events
  useEffect(() => {
    const unlisten = listen(`thinking:${sessionId}`, (event) => {
      setThinking(prev => [...prev, event.payload as ThinkingBlock]);
    });

    // Ctrl+O toggle
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key === 'o') {
        e.preventDefault();
        setShowThinking(prev => !prev);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      unlisten.then(fn => fn());
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [sessionId]);

  if (!showThinking) return null;

  return (
    <div className="thinking-transcript">
      <div className="thinking-header">
        <span>Claude's Thinking</span>
        <kbd>Ctrl+O</kbd> to toggle
      </div>
      <div className="thinking-content">
        {thinking.map((block, i) => (
          <ThinkingBlockView key={i} block={block} />
        ))}
      </div>
    </div>
  );
}
```

### 4.5 Structured Output Support

**JSON Schema validation for outputs**:

```rust
pub struct StructuredOutputManager {
    validator: jsonschema::JSONSchema,
}

impl StructuredOutputManager {
    pub fn new(schema: &Value) -> Result<Self> {
        let validator = jsonschema::JSONSchema::compile(schema)
            .map_err(|e| anyhow!("Invalid JSON schema: {}", e))?;
        Ok(Self { validator })
    }

    pub fn validate(&self, output: &Value) -> Result<()> {
        let result = self.validator.validate(output);
        if let Err(errors) = result {
            let error_messages: Vec<String> = errors
                .map(|e| format!("{}: {}", e.instance_path, e))
                .collect();
            return Err(anyhow!("Validation errors: {}", error_messages.join(", ")));
        }
        Ok(())
    }

    pub async fn execute_with_schema(
        &self,
        prompt: &str,
        schema: &Value,
    ) -> Result<Value> {
        let args = vec![
            "-p".to_string(),
            prompt.to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--json-schema".to_string(),
            serde_json::to_string(schema)?,
        ];

        let output = self.execute_claude(args).await?;
        let parsed: Value = serde_json::from_str(&output)?;

        // Validate against schema
        self.validate(&parsed)?;

        Ok(parsed)
    }
}
```

---

## Phase 5: Enterprise & Production Features

### 5.1 Web Mode Authentication

**OAuth 2.1 / OIDC for web server mode**:

```rust
pub struct AuthMiddleware {
    config: AuthConfig,
    token_validator: TokenValidator,
    session_store: SessionStore,
}

pub enum AuthConfig {
    None,
    BasicAuth { users: HashMap<String, String> },
    OAuth2 {
        issuer: String,
        client_id: String,
        client_secret: Option<String>,
        redirect_uri: String,
        scopes: Vec<String>,
    },
    ApiKey { keys: HashSet<String> },
}

impl AuthMiddleware {
    pub async fn authenticate(&self, req: &Request) -> Result<AuthenticatedUser> {
        match &self.config {
            AuthConfig::None => Ok(AuthenticatedUser::anonymous()),

            AuthConfig::BasicAuth { users } => {
                let header = req.headers().get("Authorization")
                    .ok_or(AuthError::MissingCredentials)?;
                let (user, pass) = parse_basic_auth(header)?;
                if users.get(&user) == Some(&pass) {
                    Ok(AuthenticatedUser::basic(user))
                } else {
                    Err(AuthError::InvalidCredentials)
                }
            }

            AuthConfig::OAuth2 { .. } => {
                let token = extract_bearer_token(req)?;
                let claims = self.token_validator.validate(&token).await?;
                Ok(AuthenticatedUser::oauth(claims))
            }

            AuthConfig::ApiKey { keys } => {
                let key = req.headers().get("X-API-Key")
                    .ok_or(AuthError::MissingApiKey)?
                    .to_str()?;
                if keys.contains(key) {
                    Ok(AuthenticatedUser::api_key(key))
                } else {
                    Err(AuthError::InvalidApiKey)
                }
            }
        }
    }
}
```

### 5.2 Rate Limiting & Usage Quotas

```rust
pub struct RateLimiter {
    limits: HashMap<String, RateLimit>,
    storage: Arc<dyn RateLimitStorage>,
}

pub struct RateLimit {
    requests_per_minute: u32,
    requests_per_hour: u32,
    requests_per_day: u32,
    tokens_per_day: u64,
    concurrent_sessions: u32,
}

impl RateLimiter {
    pub async fn check_and_update(&self, user_id: &str, action: &Action) -> Result<()> {
        let usage = self.storage.get_usage(user_id).await?;
        let limits = self.get_limits(user_id);

        // Check various limits
        if usage.requests_last_minute >= limits.requests_per_minute {
            return Err(RateLimitError::MinuteLimit);
        }
        if usage.requests_last_hour >= limits.requests_per_hour {
            return Err(RateLimitError::HourLimit);
        }
        if usage.requests_today >= limits.requests_per_day {
            return Err(RateLimitError::DayLimit);
        }
        if usage.tokens_today >= limits.tokens_per_day {
            return Err(RateLimitError::TokenLimit);
        }
        if usage.active_sessions >= limits.concurrent_sessions {
            return Err(RateLimitError::ConcurrentSessionLimit);
        }

        // Update usage
        self.storage.increment_usage(user_id, action).await?;

        Ok(())
    }
}
```

### 5.3 Audit Logging

```rust
pub struct AuditLogger {
    storage: Arc<dyn AuditStorage>,
}

pub struct AuditEvent {
    id: String,
    timestamp: DateTime<Utc>,
    user_id: String,
    session_id: Option<String>,
    action: AuditAction,
    resource: String,
    outcome: AuditOutcome,
    metadata: HashMap<String, Value>,
    ip_address: Option<IpAddr>,
    user_agent: Option<String>,
}

pub enum AuditAction {
    // Session actions
    SessionStart,
    SessionEnd,
    SessionCancel,

    // Claude actions
    PromptSubmit,
    ToolUse { tool: String },
    FileRead { path: String },
    FileWrite { path: String },
    BashCommand { command: String },

    // MCP actions
    McpServerConnect { server: String },
    McpToolCall { server: String, tool: String },

    // Admin actions
    SettingsChange,
    PermissionChange,
    UserCreate,
    UserDelete,
}

impl AuditLogger {
    pub async fn log(&self, event: AuditEvent) -> Result<()> {
        // Store to database
        self.storage.store(&event).await?;

        // Real-time streaming to external systems
        if let Some(webhook) = &self.webhook {
            webhook.send(&event).await?;
        }

        // Alert on sensitive actions
        if event.is_sensitive() {
            self.alert_security_team(&event).await?;
        }

        Ok(())
    }
}
```

### 5.4 Multi-User Support

```rust
pub struct UserManager {
    db: DatabasePool,
    auth: AuthMiddleware,
}

pub struct User {
    id: String,
    email: String,
    name: String,
    role: UserRole,
    created_at: DateTime<Utc>,
    last_login: Option<DateTime<Utc>>,
    settings: UserSettings,
    quotas: UserQuotas,
}

pub enum UserRole {
    Admin,
    Developer,
    Viewer,
}

pub struct UserSettings {
    default_model: String,
    theme: String,
    notifications: NotificationSettings,
    mcp_servers: Vec<String>,  // Allowed MCP servers
}
```

### 5.5 Team Workspaces

```rust
pub struct Workspace {
    id: String,
    name: String,
    owner_id: String,
    members: Vec<WorkspaceMember>,
    settings: WorkspaceSettings,
    shared_resources: SharedResources,
}

pub struct SharedResources {
    agents: Vec<String>,          // Shared agent IDs
    mcp_servers: Vec<String>,     // Shared MCP server IDs
    slash_commands: Vec<String>,  // Shared command IDs
    workflows: Vec<String>,       // Shared workflow IDs
}

pub struct WorkspaceSettings {
    default_permissions: PermissionSet,
    allowed_models: Vec<String>,
    rate_limits: RateLimit,
    audit_level: AuditLevel,
}
```

---

## Phase 6: Advanced UX & Intelligence

### 6.1 Smart Command Palette

**Unified command palette with fuzzy search**:

```typescript
// src/components/CommandPalette.tsx
export function CommandPalette() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<CommandResult[]>([]);

  const sources: CommandSource[] = [
    new SlashCommandSource(),
    new AgentSource(),
    new McpToolSource(),
    new WorkflowSource(),
    new SettingsSource(),
    new RecentSessionSource(),
    new FileSource(),
  ];

  useEffect(() => {
    if (!query) {
      setResults(getRecentCommands());
      return;
    }

    // Fuzzy search across all sources
    const allResults = sources.flatMap(source =>
      source.search(query)
    );

    // Sort by relevance
    setResults(sortByRelevance(allResults, query));
  }, [query]);

  return (
    <Dialog open={open}>
      <div className="command-palette">
        <Input
          placeholder="Type a command or search..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          autoFocus
        />
        <CommandResultList
          results={results}
          onSelect={executeCommand}
        />
        <CommandPaletteFooter>
          <kbd>↑↓</kbd> Navigate
          <kbd>Enter</kbd> Execute
          <kbd>Esc</kbd> Close
        </CommandPaletteFooter>
      </div>
    </Dialog>
  );
}
```

### 6.2 Intelligent Session Insights

```typescript
// src/components/SessionInsights.tsx
export function SessionInsights({ sessionId }: Props) {
  const insights = useSessionInsights(sessionId);

  return (
    <div className="session-insights">
      {/* Token Usage */}
      <InsightCard title="Token Usage">
        <TokenUsageChart
          input={insights.inputTokens}
          output={insights.outputTokens}
          cached={insights.cachedTokens}
        />
        <CostEstimate cost={insights.estimatedCost} />
      </InsightCard>

      {/* Files Modified */}
      <InsightCard title="Files Modified">
        <FileTreeDiff files={insights.modifiedFiles} />
      </InsightCard>

      {/* Tools Used */}
      <InsightCard title="Tools Used">
        <ToolUsageBreakdown tools={insights.toolUsage} />
      </InsightCard>

      {/* Session Timeline */}
      <InsightCard title="Timeline">
        <SessionTimeline
          events={insights.events}
          checkpoints={insights.checkpoints}
        />
      </InsightCard>

      {/* AI-Generated Summary */}
      <InsightCard title="Session Summary">
        <AiSummary summary={insights.aiSummary} />
      </InsightCard>
    </div>
  );
}
```

### 6.3 Predictive Suggestions

```typescript
// AI-powered suggestions based on context
export function useContextualSuggestions(context: SessionContext) {
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);

  useEffect(() => {
    const generateSuggestions = async () => {
      // Analyze current context
      const analysis = await analyzeContext(context);

      // Generate suggestions based on:
      // 1. Current conversation state
      // 2. Project structure
      // 3. Recent actions
      // 4. Common patterns

      const newSuggestions: Suggestion[] = [];

      // Suggest next actions
      if (analysis.likelyNextAction) {
        newSuggestions.push({
          type: 'action',
          title: analysis.likelyNextAction,
          confidence: analysis.confidence,
        });
      }

      // Suggest relevant tools
      if (analysis.relevantTools.length > 0) {
        newSuggestions.push({
          type: 'tools',
          title: 'Suggested tools',
          items: analysis.relevantTools,
        });
      }

      // Suggest from history
      const historySuggestions = await getHistorySuggestions(context);
      newSuggestions.push(...historySuggestions);

      setSuggestions(newSuggestions);
    };

    generateSuggestions();
  }, [context]);

  return suggestions;
}
```

### 6.4 Voice Input Support

```typescript
// src/hooks/useVoiceInput.ts
export function useVoiceInput() {
  const [isListening, setIsListening] = useState(false);
  const [transcript, setTranscript] = useState('');

  const recognition = useMemo(() => {
    if ('webkitSpeechRecognition' in window) {
      const rec = new webkitSpeechRecognition();
      rec.continuous = true;
      rec.interimResults = true;
      return rec;
    }
    return null;
  }, []);

  const startListening = useCallback(() => {
    if (recognition) {
      recognition.start();
      setIsListening(true);
    }
  }, [recognition]);

  const stopListening = useCallback(() => {
    if (recognition) {
      recognition.stop();
      setIsListening(false);
    }
  }, [recognition]);

  useEffect(() => {
    if (recognition) {
      recognition.onresult = (event) => {
        const current = event.resultIndex;
        const transcript = event.results[current][0].transcript;
        setTranscript(transcript);
      };
    }
  }, [recognition]);

  return { isListening, transcript, startListening, stopListening };
}
```

---

## Architecture Diagrams

### Overall System Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                            OPCODE 2.0                                   │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                     FRONTEND (React + TypeScript)                │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │   │
│  │  │ Sessions │ │  Skills  │ │   MCP    │ │ Workflow │           │   │
│  │  │ Manager  │ │ Manager  │ │ Manager  │ │  Builder │           │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │   │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │   │
│  │  │  Agents  │ │Checkpoint│ │ Insights │ │ Settings │           │   │
│  │  │  Studio  │ │ Timeline │ │Dashboard │ │  Panel   │           │   │
│  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                   │                                     │
│                                   ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                     API ADAPTER LAYER                            │   │
│  │         (Unified API for Tauri IPC / Web REST / WebSocket)       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                   │                                     │
│         ┌─────────────────────────┼─────────────────────────┐          │
│         ▼                         ▼                         ▼          │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐    │
│  │   TAURI     │          │ WEB SERVER  │          │  WEBSOCKET  │    │
│  │   MODE      │          │    MODE     │          │   GATEWAY   │    │
│  └─────────────┘          └─────────────┘          └─────────────┘    │
│         │                         │                         │          │
│         └─────────────────────────┼─────────────────────────┘          │
│                                   ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      RUST BACKEND CORE                           │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐        │   │
│  │  │    Session    │  │    Process    │  │      MCP      │        │   │
│  │  │    Manager    │  │   Registry    │  │   Transport   │        │   │
│  │  └───────────────┘  └───────────────┘  └───────────────┘        │   │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐        │   │
│  │  │   Checkpoint  │  │     Skill     │  │     Auth      │        │   │
│  │  │    Manager    │  │    Engine     │  │   Middleware  │        │   │
│  │  └───────────────┘  └───────────────┘  └───────────────┘        │   │
│  │  ┌───────────────┐  ┌───────────────┐  ┌───────────────┐        │   │
│  │  │   Database    │  │    Audit      │  │     Rate      │        │   │
│  │  │     Pool      │  │    Logger     │  │   Limiter     │        │   │
│  │  └───────────────┘  └───────────────┘  └───────────────┘        │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                   │                                     │
│                                   ▼                                     │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    EXTERNAL INTEGRATIONS                         │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐ │   │
│  │  │   Claude   │  │   Remote   │  │    Git     │  │  External  │ │   │
│  │  │   Code     │  │    MCP     │  │Integration │  │   APIs     │ │   │
│  │  │   CLI      │  │  Servers   │  │            │  │            │ │   │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘ │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### MCP Streamable HTTP Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    MCP STREAMABLE HTTP TRANSPORT                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────┐                              ┌─────────────────────┐  │
│  │   OPCODE    │                              │   REMOTE VPS        │  │
│  │   CLIENT    │                              │   MCP SERVER        │  │
│  ├─────────────┤                              ├─────────────────────┤  │
│  │             │  ──── POST /mcp ──────────▶  │                     │  │
│  │  Transport  │       (JSON-RPC Request)     │   StreamableHTTP    │  │
│  │   Layer     │                              │     Endpoint        │  │
│  │             │  ◀─── Response ───────────── │                     │  │
│  │             │       (JSON / SSE Stream)    │                     │  │
│  ├─────────────┤                              ├─────────────────────┤  │
│  │             │                              │                     │  │
│  │   OAuth     │  ──── Authorization ──────▶  │   OAuth 2.1         │  │
│  │   Client    │       (PKCE Flow)            │   Provider          │  │
│  │             │                              │                     │  │
│  ├─────────────┤                              ├─────────────────────┤  │
│  │             │                              │                     │  │
│  │   Session   │  ◀─── Mcp-Session-Id ─────── │   Session           │  │
│  │   Manager   │       (Header)               │   Manager           │  │
│  │             │                              │                     │  │
│  └─────────────┘                              └─────────────────────┘  │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                    PROTOCOL FLOW                                 │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  1. Initialize:                                                  │   │
│  │     POST /mcp { "method": "initialize", "params": {...} }        │   │
│  │     ◀─── { "result": {...}, headers: "Mcp-Session-Id: xxx" }     │   │
│  │                                                                  │   │
│  │  2. List Tools:                                                  │   │
│  │     POST /mcp { "method": "tools/list" }                         │   │
│  │     ◀─── { "result": { "tools": [...] } }                        │   │
│  │                                                                  │   │
│  │  3. Call Tool (Streaming):                                       │   │
│  │     POST /mcp { "method": "tools/call", "params": {...} }        │   │
│  │     ◀─── SSE: data: {"type": "progress", ...}                    │   │
│  │     ◀─── SSE: data: {"type": "progress", ...}                    │   │
│  │     ◀─── SSE: data: {"type": "result", ...}                      │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Skills & Extensions Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    SKILLS & EXTENSIONS ECOSYSTEM                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                      SKILL MANAGER                               │   │
│  │                                                                  │   │
│  │  ┌──────────────────────────────────────────────────────────┐   │   │
│  │  │                    SKILL REGISTRY                         │   │   │
│  │  ├──────────────────────────────────────────────────────────┤   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐    │   │   │
│  │  │  │  Slash   │ │  Custom  │ │   MCP    │ │  Hooks   │    │   │   │
│  │  │  │ Commands │ │  Agents  │ │ Servers  │ │          │    │   │   │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘    │   │   │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐    │   │   │
│  │  │  │ Plugins  │ │Templates │ │Workflows │ │  Custom  │    │   │   │
│  │  │  │          │ │          │ │          │ │  Tools   │    │   │   │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘    │   │   │
│  │  └──────────────────────────────────────────────────────────┘   │   │
│  │                              │                                   │   │
│  │                              ▼                                   │   │
│  │  ┌──────────────────────────────────────────────────────────┐   │   │
│  │  │                   SKILL ENGINE                            │   │   │
│  │  ├──────────────────────────────────────────────────────────┤   │   │
│  │  │  ┌────────────┐  ┌────────────┐  ┌────────────┐          │   │   │
│  │  │  │   Loader   │  │  Executor  │  │  Validator │          │   │   │
│  │  │  └────────────┘  └────────────┘  └────────────┘          │   │   │
│  │  └──────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │                       SKILL SOURCES                              │   │
│  ├─────────────────────────────────────────────────────────────────┤   │
│  │                                                                  │   │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────┐ │   │
│  │  │   Local    │  │   GitHub   │  │  Registry  │  │    URL     │ │   │
│  │  │   Files    │  │   Repos    │  │  (Public)  │  │  (Direct)  │ │   │
│  │  └────────────┘  └────────────┘  └────────────┘  └────────────┘ │   │
│  │       │               │               │               │         │   │
│  │       └───────────────┴───────────────┴───────────────┘         │   │
│  │                              │                                   │   │
│  │                              ▼                                   │   │
│  │  ┌──────────────────────────────────────────────────────────┐   │   │
│  │  │                   SKILL CREATOR                           │   │   │
│  │  ├──────────────────────────────────────────────────────────┤   │   │
│  │  │  ┌────────────┐  ┌────────────┐  ┌────────────┐          │   │   │
│  │  │  │  Visual    │  │   Code     │  │  Template  │          │   │   │
│  │  │  │  Builder   │  │   Editor   │  │  Wizard    │          │   │   │
│  │  │  └────────────┘  └────────────┘  └────────────┘          │   │   │
│  │  └──────────────────────────────────────────────────────────┘   │   │
│  │                                                                  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Implementation Priority Matrix

### Priority 1: Critical (Immediate)
| Feature | Impact | Effort | Dependencies |
|---------|--------|--------|--------------|
| Session Isolation | Critical | Medium | None |
| Process Cancellation | Critical | Low | Session Isolation |
| stderr Capture | High | Low | None |
| Database Connection Pooling | High | Medium | None |

### Priority 2: High (Short-term)
| Feature | Impact | Effort | Dependencies |
|---------|--------|--------|--------------|
| MCP Streamable HTTP | Critical | High | None |
| Remote MCP Management | High | Medium | Streamable HTTP |
| Concurrent Sessions | High | Medium | Session Isolation |
| OAuth 2.1 for MCP | High | High | Streamable HTTP |

### Priority 3: Medium (Medium-term)
| Feature | Impact | Effort | Dependencies |
|---------|--------|--------|--------------|
| Skills Manager | High | High | None |
| Visual Skill Creator | Medium | High | Skills Manager |
| Enhanced Checkpoints | Medium | Medium | None |
| Parallel Tasks | Medium | Medium | Concurrent Sessions |
| Thinking Display | Medium | Low | None |

### Priority 4: Enhancement (Long-term)
| Feature | Impact | Effort | Dependencies |
|---------|--------|--------|--------------|
| Workflow Builder | Medium | Very High | Skills Manager |
| Web Authentication | High | High | None |
| Multi-User Support | Medium | Very High | Web Auth |
| Team Workspaces | Medium | High | Multi-User |
| Voice Input | Low | Medium | None |

---

## Success Metrics

### Performance Targets
- Session startup time: < 500ms
- MCP connection time: < 1s
- Checkpoint creation: < 2s
- UI response time: < 100ms

### Reliability Targets
- Uptime: 99.9%
- Process cleanup rate: 100%
- Data integrity: 100%
- Error recovery: 95%

### User Experience Targets
- Feature discoverability: > 80%
- Task completion rate: > 90%
- User satisfaction: > 4.5/5

---

## Conclusion

This master plan transforms Opcode from a capable wrapper into the **definitive Claude Code experience**. By addressing all 45+ identified limitations and implementing cutting-edge features like:

- **MCP Streamable HTTP** for remote server support
- **Visual Skills Ecosystem** for easy extension creation
- **Enhanced Process Management** for reliability
- **Enterprise Features** for production deployment
- **Intelligent UX** for delightful experience

Opcode 2.0 "Apex" will set the standard for AI coding assistant interfaces.

---

*Document Version: 1.0*
*Last Updated: 2026-01-15*
*Author: Claude Opus 4.5*
