/**
 * Enhanced Agent Builder
 *
 * Opcode 2.0 - Visual builder for creating and editing custom agents.
 * Features:
 * - Subagent configuration
 * - Permission rules editor (allow/deny)
 * - MCP server selector
 * - Hook configuration
 * - Model selection with Opus 4.5 default
 */

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";

// Types
interface Agent {
  id: number | null;
  name: string;
  icon: string;
  system_prompt: string;
  default_task: string | null;
  model: string;
  enable_file_read: boolean;
  enable_file_write: boolean;
  enable_network: boolean;
  hooks: string | null;
  description: string | null;
  subagents: string | null;
  permission_mode: string | null;
  allow_rules: string | null;
  deny_rules: string | null;
  mcp_servers: string | null;
  fallback_model: string | null;
  max_turns: number | null;
  is_opus_optimized: boolean | null;
}

interface SubagentDef {
  name: string;
  description: string;
  system_prompt: string;
  tools: string[];
  model: string;
}

interface PermissionRule {
  tool: string;
  matcher: string;
  description?: string;
}

interface RemoteMcpServer {
  id: string;
  name: string;
  status: string;
}

// Available icons
const AGENT_ICONS = [
  "🤖", "🔧", "📝", "🎯", "💡", "🚀", "⚡", "🔍", "📊", "🛠️",
  "🎨", "📚", "🧪", "🔬", "🏗️", "🎮", "🌐", "💻", "📱", "🔐",
];

// Available models
const MODELS = [
  { value: "opus", label: "Claude Opus 4.5 (Recommended)", description: "Most capable" },
  { value: "sonnet", label: "Claude Sonnet 4.5", description: "Balanced" },
  { value: "haiku", label: "Claude Haiku 3.5", description: "Fast" },
];

// Permission modes
const PERMISSION_MODES = [
  { value: "default", label: "Default", description: "Ask before non-read operations" },
  { value: "plan", label: "Plan Only", description: "Analyze only, no modifications" },
  { value: "acceptEdits", label: "Accept Edits", description: "Auto-approve file edits" },
  { value: "bypassPermissions", label: "Bypass", description: "Full autonomy (use cautiously)" },
];

// Subagent Editor
function SubagentEditor({
  subagents,
  onChange,
}: {
  subagents: SubagentDef[];
  onChange: (subagents: SubagentDef[]) => void;
}) {
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const [newSubagent, setNewSubagent] = useState<SubagentDef>({
    name: "",
    description: "",
    system_prompt: "",
    tools: [],
    model: "sonnet",
  });

  const addSubagent = () => {
    if (newSubagent.name && newSubagent.system_prompt) {
      onChange([...subagents, newSubagent]);
      setNewSubagent({
        name: "",
        description: "",
        system_prompt: "",
        tools: [],
        model: "sonnet",
      });
    }
  };

  const removeSubagent = (index: number) => {
    onChange(subagents.filter((_, i) => i !== index));
  };

  return (
    <div className="space-y-4">
      <div className="text-sm text-muted-foreground">
        Define specialized subagents that this agent can delegate tasks to.
      </div>

      {subagents.length > 0 && (
        <div className="space-y-2">
          {subagents.map((sub, index) => (
            <Card key={index} className="p-3">
              <div className="flex items-center justify-between">
                <div>
                  <span className="font-medium">{sub.name}</span>
                  <span className="text-sm text-muted-foreground ml-2">
                    ({sub.model})
                  </span>
                </div>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => removeSubagent(index)}
                >
                  Remove
                </Button>
              </div>
              {sub.description && (
                <p className="text-sm text-muted-foreground mt-1">
                  {sub.description}
                </p>
              )}
            </Card>
          ))}
        </div>
      )}

      <Card className="p-4">
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <Label>Name</Label>
              <Input
                value={newSubagent.name}
                onChange={(e) =>
                  setNewSubagent({ ...newSubagent, name: e.target.value })
                }
                placeholder="code-reviewer"
              />
            </div>
            <div>
              <Label>Model</Label>
              <Select
                value={newSubagent.model}
                onValueChange={(v) =>
                  setNewSubagent({ ...newSubagent, model: v })
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {MODELS.map((m) => (
                    <SelectItem key={m.value} value={m.value}>
                      {m.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>
          <div>
            <Label>Description</Label>
            <Input
              value={newSubagent.description}
              onChange={(e) =>
                setNewSubagent({ ...newSubagent, description: e.target.value })
              }
              placeholder="Reviews code for quality and security"
            />
          </div>
          <div>
            <Label>System Prompt</Label>
            <Textarea
              value={newSubagent.system_prompt}
              onChange={(e) =>
                setNewSubagent({ ...newSubagent, system_prompt: e.target.value })
              }
              placeholder="You are a code reviewer. Focus on..."
              rows={3}
            />
          </div>
          <Button onClick={addSubagent} size="sm">
            Add Subagent
          </Button>
        </div>
      </Card>
    </div>
  );
}

// Permission Rules Editor
function PermissionRulesEditor({
  allowRules,
  denyRules,
  onAllowChange,
  onDenyChange,
}: {
  allowRules: PermissionRule[];
  denyRules: PermissionRule[];
  onAllowChange: (rules: PermissionRule[]) => void;
  onDenyChange: (rules: PermissionRule[]) => void;
}) {
  const [newRule, setNewRule] = useState({ tool: "", matcher: "", type: "allow" });

  const addRule = () => {
    if (newRule.tool && newRule.matcher) {
      const rule: PermissionRule = { tool: newRule.tool, matcher: newRule.matcher };
      if (newRule.type === "allow") {
        onAllowChange([...allowRules, rule]);
      } else {
        onDenyChange([...denyRules, rule]);
      }
      setNewRule({ tool: "", matcher: "", type: "allow" });
    }
  };

  const removeAllowRule = (index: number) => {
    onAllowChange(allowRules.filter((_, i) => i !== index));
  };

  const removeDenyRule = (index: number) => {
    onDenyChange(denyRules.filter((_, i) => i !== index));
  };

  return (
    <div className="space-y-4">
      <div className="text-sm text-muted-foreground">
        Define which tools this agent can or cannot use.
      </div>

      <div className="grid grid-cols-2 gap-4">
        <Card className="p-3">
          <h4 className="font-medium text-green-600 mb-2">Allow Rules</h4>
          {allowRules.length === 0 ? (
            <p className="text-sm text-muted-foreground">No allow rules</p>
          ) : (
            <div className="space-y-1">
              {allowRules.map((rule, i) => (
                <div key={i} className="flex items-center justify-between text-sm">
                  <code className="bg-muted px-1 rounded">
                    {rule.tool}: {rule.matcher}
                  </code>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => removeAllowRule(i)}
                  >
                    ×
                  </Button>
                </div>
              ))}
            </div>
          )}
        </Card>

        <Card className="p-3">
          <h4 className="font-medium text-red-600 mb-2">Deny Rules</h4>
          {denyRules.length === 0 ? (
            <p className="text-sm text-muted-foreground">No deny rules</p>
          ) : (
            <div className="space-y-1">
              {denyRules.map((rule, i) => (
                <div key={i} className="flex items-center justify-between text-sm">
                  <code className="bg-muted px-1 rounded">
                    {rule.tool}: {rule.matcher}
                  </code>
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => removeDenyRule(i)}
                  >
                    ×
                  </Button>
                </div>
              ))}
            </div>
          )}
        </Card>
      </div>

      <Card className="p-3">
        <div className="grid grid-cols-4 gap-2">
          <Select
            value={newRule.type}
            onValueChange={(v) => setNewRule({ ...newRule, type: v })}
          >
            <SelectTrigger>
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="allow">Allow</SelectItem>
              <SelectItem value="deny">Deny</SelectItem>
            </SelectContent>
          </Select>
          <Input
            placeholder="Tool (e.g., Bash)"
            value={newRule.tool}
            onChange={(e) => setNewRule({ ...newRule, tool: e.target.value })}
          />
          <Input
            placeholder="Matcher (e.g., npm run*)"
            value={newRule.matcher}
            onChange={(e) => setNewRule({ ...newRule, matcher: e.target.value })}
          />
          <Button onClick={addRule}>Add</Button>
        </div>
      </Card>
    </div>
  );
}

// MCP Server Selector
function McpServerSelector({
  selectedServers,
  onChange,
}: {
  selectedServers: string[];
  onChange: (servers: string[]) => void;
}) {
  const [availableServers, setAvailableServers] = useState<RemoteMcpServer[]>([]);

  useEffect(() => {
    loadServers();
  }, []);

  const loadServers = async () => {
    try {
      const servers = await invoke<RemoteMcpServer[]>("list_remote_mcp_servers");
      setAvailableServers(servers);
    } catch (e) {
      console.error("Failed to load MCP servers:", e);
    }
  };

  const toggleServer = (serverId: string) => {
    if (selectedServers.includes(serverId)) {
      onChange(selectedServers.filter((id) => id !== serverId));
    } else {
      onChange([...selectedServers, serverId]);
    }
  };

  return (
    <div className="space-y-4">
      <div className="text-sm text-muted-foreground">
        Select which MCP servers this agent can access.
      </div>

      {availableServers.length === 0 ? (
        <Card className="p-4 text-center text-muted-foreground">
          No remote MCP servers configured.{" "}
          <a href="#" className="text-primary">
            Add a server
          </a>
        </Card>
      ) : (
        <div className="grid grid-cols-2 gap-2">
          {availableServers.map((server) => (
            <Card
              key={server.id}
              className={`p-3 cursor-pointer transition-colors ${
                selectedServers.includes(server.id)
                  ? "border-primary bg-primary/5"
                  : "hover:bg-muted/50"
              }`}
              onClick={() => toggleServer(server.id)}
            >
              <div className="flex items-center gap-2">
                <div
                  className={`w-2 h-2 rounded-full ${
                    server.status === "connected"
                      ? "bg-green-500"
                      : "bg-gray-400"
                  }`}
                />
                <span className="font-medium">{server.name}</span>
                {selectedServers.includes(server.id) && (
                  <Badge variant="default" className="ml-auto">
                    Selected
                  </Badge>
                )}
              </div>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}

// Main Agent Builder Component
export function AgentBuilder({
  agent,
  onSave,
  onCancel,
}: {
  agent?: Agent | null;
  onSave: (agent: Agent) => void;
  onCancel: () => void;
}) {
  const isEditing = agent?.id != null;

  // Form state
  const [name, setName] = useState(agent?.name || "");
  const [icon, setIcon] = useState(agent?.icon || "🤖");
  const [description, setDescription] = useState(agent?.description || "");
  const [systemPrompt, setSystemPrompt] = useState(agent?.system_prompt || "");
  const [defaultTask, setDefaultTask] = useState(agent?.default_task || "");
  const [model, setModel] = useState(agent?.model || "opus");
  const [fallbackModel, setFallbackModel] = useState(agent?.fallback_model || "sonnet");
  const [permissionMode, setPermissionMode] = useState(agent?.permission_mode || "default");
  const [maxTurns, setMaxTurns] = useState(agent?.max_turns || 0);
  const [enableFileRead, setEnableFileRead] = useState(agent?.enable_file_read ?? true);
  const [enableFileWrite, setEnableFileWrite] = useState(agent?.enable_file_write ?? true);
  const [enableNetwork, setEnableNetwork] = useState(agent?.enable_network ?? false);
  const [isOpusOptimized, setIsOpusOptimized] = useState(agent?.is_opus_optimized ?? true);

  // Parsed JSON fields
  const [subagents, setSubagents] = useState<SubagentDef[]>(() => {
    try {
      return agent?.subagents ? JSON.parse(agent.subagents) : [];
    } catch {
      return [];
    }
  });

  const [allowRules, setAllowRules] = useState<PermissionRule[]>(() => {
    try {
      return agent?.allow_rules ? JSON.parse(agent.allow_rules) : [];
    } catch {
      return [];
    }
  });

  const [denyRules, setDenyRules] = useState<PermissionRule[]>(() => {
    try {
      return agent?.deny_rules ? JSON.parse(agent.deny_rules) : [];
    } catch {
      return [];
    }
  });

  const [mcpServers, setMcpServers] = useState<string[]>(() => {
    try {
      return agent?.mcp_servers ? JSON.parse(agent.mcp_servers) : [];
    } catch {
      return [];
    }
  });

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSave = async () => {
    if (!name.trim()) {
      setError("Name is required");
      return;
    }
    if (!systemPrompt.trim()) {
      setError("System prompt is required");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const agentData: Agent = {
        id: agent?.id || null,
        name: name.trim(),
        icon,
        system_prompt: systemPrompt.trim(),
        default_task: defaultTask.trim() || null,
        model,
        enable_file_read: enableFileRead,
        enable_file_write: enableFileWrite,
        enable_network: enableNetwork,
        hooks: agent?.hooks || null,
        description: description.trim() || null,
        subagents: subagents.length > 0 ? JSON.stringify(subagents) : null,
        permission_mode: permissionMode,
        allow_rules: allowRules.length > 0 ? JSON.stringify(allowRules) : null,
        deny_rules: denyRules.length > 0 ? JSON.stringify(denyRules) : null,
        mcp_servers: mcpServers.length > 0 ? JSON.stringify(mcpServers) : null,
        fallback_model: fallbackModel || null,
        max_turns: maxTurns > 0 ? maxTurns : null,
        is_opus_optimized: isOpusOptimized,
      };

      onSave(agentData);
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">
            {isEditing ? "Edit Agent" : "Create Agent"}
          </h2>
          <p className="text-muted-foreground">
            Build a custom agent powered by Claude
          </p>
        </div>
      </div>

      {error && (
        <div className="text-sm text-red-500 bg-red-50 dark:bg-red-900/20 p-3 rounded">
          {error}
        </div>
      )}

      <Tabs defaultValue="basic" className="space-y-4">
        <TabsList className="grid w-full grid-cols-5">
          <TabsTrigger value="basic">Basic</TabsTrigger>
          <TabsTrigger value="permissions">Permissions</TabsTrigger>
          <TabsTrigger value="subagents">Subagents</TabsTrigger>
          <TabsTrigger value="mcp">MCP Servers</TabsTrigger>
          <TabsTrigger value="advanced">Advanced</TabsTrigger>
        </TabsList>

        {/* Basic Tab */}
        <TabsContent value="basic" className="space-y-4">
          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Icon</Label>
              <div className="flex flex-wrap gap-1">
                {AGENT_ICONS.map((i) => (
                  <button
                    key={i}
                    className={`text-xl p-1 rounded hover:bg-muted ${
                      icon === i ? "bg-primary/20 ring-2 ring-primary" : ""
                    }`}
                    onClick={() => setIcon(i)}
                  >
                    {i}
                  </button>
                ))}
              </div>
            </div>
            <div className="space-y-2">
              <Label htmlFor="name">Name</Label>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Code Assistant"
              />
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="description">Description</Label>
            <Input
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="A helpful assistant for coding tasks"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label>Model</Label>
              <Select value={model} onValueChange={setModel}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {MODELS.map((m) => (
                    <SelectItem key={m.value} value={m.value}>
                      <div className="flex flex-col">
                        <span>{m.label}</span>
                        <span className="text-xs text-muted-foreground">
                          {m.description}
                        </span>
                      </div>
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="space-y-2">
              <Label>Fallback Model</Label>
              <Select value={fallbackModel} onValueChange={setFallbackModel}>
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  {MODELS.map((m) => (
                    <SelectItem key={m.value} value={m.value}>
                      {m.label}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="systemPrompt">System Prompt</Label>
            <Textarea
              id="systemPrompt"
              value={systemPrompt}
              onChange={(e) => setSystemPrompt(e.target.value)}
              placeholder="You are a helpful coding assistant..."
              rows={8}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="defaultTask">Default Task (optional)</Label>
            <Input
              id="defaultTask"
              value={defaultTask}
              onChange={(e) => setDefaultTask(e.target.value)}
              placeholder="Review the current codebase"
            />
          </div>
        </TabsContent>

        {/* Permissions Tab */}
        <TabsContent value="permissions" className="space-y-4">
          <Card className="p-4">
            <h3 className="font-medium mb-4">Permission Mode</h3>
            <Select value={permissionMode} onValueChange={setPermissionMode}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PERMISSION_MODES.map((pm) => (
                  <SelectItem key={pm.value} value={pm.value}>
                    <div className="flex flex-col">
                      <span>{pm.label}</span>
                      <span className="text-xs text-muted-foreground">
                        {pm.description}
                      </span>
                    </div>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Card>

          <Card className="p-4">
            <h3 className="font-medium mb-4">Basic Permissions</h3>
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <div>
                  <Label>File Read</Label>
                  <p className="text-sm text-muted-foreground">
                    Allow reading files from disk
                  </p>
                </div>
                <Switch
                  checked={enableFileRead}
                  onCheckedChange={setEnableFileRead}
                />
              </div>
              <div className="flex items-center justify-between">
                <div>
                  <Label>File Write</Label>
                  <p className="text-sm text-muted-foreground">
                    Allow writing and editing files
                  </p>
                </div>
                <Switch
                  checked={enableFileWrite}
                  onCheckedChange={setEnableFileWrite}
                />
              </div>
              <div className="flex items-center justify-between">
                <div>
                  <Label>Network Access</Label>
                  <p className="text-sm text-muted-foreground">
                    Allow web requests and fetching
                  </p>
                </div>
                <Switch
                  checked={enableNetwork}
                  onCheckedChange={setEnableNetwork}
                />
              </div>
            </div>
          </Card>

          <Card className="p-4">
            <h3 className="font-medium mb-4">Tool Permission Rules</h3>
            <PermissionRulesEditor
              allowRules={allowRules}
              denyRules={denyRules}
              onAllowChange={setAllowRules}
              onDenyChange={setDenyRules}
            />
          </Card>
        </TabsContent>

        {/* Subagents Tab */}
        <TabsContent value="subagents">
          <Card className="p-4">
            <h3 className="font-medium mb-4">Subagents</h3>
            <SubagentEditor
              subagents={subagents}
              onChange={setSubagents}
            />
          </Card>
        </TabsContent>

        {/* MCP Servers Tab */}
        <TabsContent value="mcp">
          <Card className="p-4">
            <h3 className="font-medium mb-4">MCP Server Integration</h3>
            <McpServerSelector
              selectedServers={mcpServers}
              onChange={setMcpServers}
            />
          </Card>
        </TabsContent>

        {/* Advanced Tab */}
        <TabsContent value="advanced" className="space-y-4">
          <Card className="p-4">
            <h3 className="font-medium mb-4">Execution Limits</h3>
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="maxTurns">Max Turns (0 = unlimited)</Label>
                <Input
                  id="maxTurns"
                  type="number"
                  min={0}
                  value={maxTurns}
                  onChange={(e) => setMaxTurns(parseInt(e.target.value) || 0)}
                />
                <p className="text-sm text-muted-foreground">
                  Limit the number of agentic turns per execution
                </p>
              </div>
            </div>
          </Card>

          <Card className="p-4">
            <h3 className="font-medium mb-4">Optimization</h3>
            <div className="flex items-center justify-between">
              <div>
                <Label>Opus 4.5 Optimized</Label>
                <p className="text-sm text-muted-foreground">
                  Enable optimizations for Opus 4.5 model
                </p>
              </div>
              <Switch
                checked={isOpusOptimized ?? false}
                onCheckedChange={setIsOpusOptimized}
              />
            </div>
          </Card>
        </TabsContent>
      </Tabs>

      <div className="flex justify-end gap-3 pt-4 border-t">
        <Button variant="outline" onClick={onCancel}>
          Cancel
        </Button>
        <Button onClick={handleSave} disabled={loading}>
          {loading ? "Saving..." : isEditing ? "Update Agent" : "Create Agent"}
        </Button>
      </div>
    </div>
  );
}

export default AgentBuilder;
