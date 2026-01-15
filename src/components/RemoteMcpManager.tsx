/**
 * Remote MCP Server Manager
 *
 * Opcode 2.0 - Manage remote MCP servers with Streamable HTTP transport.
 * Features:
 * - Add/remove remote servers
 * - Bearer token and API key authentication
 * - Health monitoring and status display
 * - Tool browsing and testing
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
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";

// Types
interface RemoteMcpServer {
  id: string;
  name: string;
  description: string | null;
  endpoint: string;
  auth_type: string;
  status: string;
  health_enabled: boolean;
  health_interval: number;
  last_health_check: string | null;
  latency_ms: number | null;
  created_at: string;
  updated_at: string;
}

interface ServerHealth {
  server_id: string;
  status: string;
  latency_ms: number | null;
  last_check: string | null;
  last_error: string | null;
  consecutive_failures: number;
  consecutive_successes: number;
  avg_latency_ms: number | null;
}

interface McpTool {
  name: string;
  description: string | null;
  input_schema: object;
}

// Status badge component
function StatusBadge({ status }: { status: string }) {
  const variants: Record<string, "default" | "secondary" | "destructive" | "outline"> = {
    connected: "default",
    healthy: "default",
    disconnected: "secondary",
    error: "destructive",
    unknown: "outline",
  };

  const colors: Record<string, string> = {
    connected: "bg-green-500",
    healthy: "bg-green-500",
    disconnected: "bg-gray-500",
    error: "bg-red-500",
    unknown: "bg-yellow-500",
  };

  return (
    <Badge variant={variants[status] || "outline"} className="gap-1">
      <span className={`w-2 h-2 rounded-full ${colors[status] || "bg-gray-400"}`} />
      {status}
    </Badge>
  );
}

// Add Server Dialog
function AddServerDialog({
  open,
  onOpenChange,
  onAdd,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: () => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [authType, setAuthType] = useState("none");
  const [token, setToken] = useState("");
  const [apiKeyHeader, setApiKeyHeader] = useState("X-API-Key");
  const [apiKeyValue, setApiKeyValue] = useState("");
  const [healthEnabled, setHealthEnabled] = useState(true);
  const [healthInterval, setHealthInterval] = useState(60);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!name.trim() || !endpoint.trim()) {
      setError("Name and endpoint are required");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await invoke("add_remote_mcp_server", {
        request: {
          name: name.trim(),
          description: description.trim() || null,
          endpoint: endpoint.trim(),
          auth_type: authType,
          token: authType === "bearer" ? token : null,
          api_key_header: authType === "api-key" ? apiKeyHeader : null,
          api_key_value: authType === "api-key" ? apiKeyValue : null,
          health_enabled: healthEnabled,
          health_interval: healthInterval,
        },
      });

      // Reset form
      setName("");
      setDescription("");
      setEndpoint("");
      setAuthType("none");
      setToken("");
      setApiKeyHeader("X-API-Key");
      setApiKeyValue("");

      onAdd();
      onOpenChange(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>Add Remote MCP Server</DialogTitle>
          <DialogDescription>
            Connect to a remote MCP server on your VPS using Streamable HTTP transport.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {error && (
            <div className="text-sm text-red-500 bg-red-50 dark:bg-red-900/20 p-2 rounded">
              {error}
            </div>
          )}

          <div className="space-y-2">
            <Label htmlFor="name">Server Name</Label>
            <Input
              id="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="My MCP Server"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="description">Description (optional)</Label>
            <Input
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Production server for..."
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="endpoint">Endpoint URL</Label>
            <Input
              id="endpoint"
              value={endpoint}
              onChange={(e) => setEndpoint(e.target.value)}
              placeholder="https://mcp.your-vps.com/mcp"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="authType">Authentication</Label>
            <Select value={authType} onValueChange={setAuthType}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">No Authentication</SelectItem>
                <SelectItem value="bearer">Bearer Token</SelectItem>
                <SelectItem value="api-key">API Key</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {authType === "bearer" && (
            <div className="space-y-2">
              <Label htmlFor="token">Bearer Token</Label>
              <Input
                id="token"
                type="password"
                value={token}
                onChange={(e) => setToken(e.target.value)}
                placeholder="your-secret-token"
              />
            </div>
          )}

          {authType === "api-key" && (
            <>
              <div className="space-y-2">
                <Label htmlFor="apiKeyHeader">Header Name</Label>
                <Input
                  id="apiKeyHeader"
                  value={apiKeyHeader}
                  onChange={(e) => setApiKeyHeader(e.target.value)}
                  placeholder="X-API-Key"
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="apiKeyValue">API Key</Label>
                <Input
                  id="apiKeyValue"
                  type="password"
                  value={apiKeyValue}
                  onChange={(e) => setApiKeyValue(e.target.value)}
                  placeholder="your-api-key"
                />
              </div>
            </>
          )}

          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <Label>Health Monitoring</Label>
              <p className="text-sm text-muted-foreground">
                Periodically check server status
              </p>
            </div>
            <Switch
              checked={healthEnabled}
              onCheckedChange={setHealthEnabled}
            />
          </div>

          {healthEnabled && (
            <div className="space-y-2">
              <Label htmlFor="healthInterval">Check Interval (seconds)</Label>
              <Input
                id="healthInterval"
                type="number"
                min={10}
                max={3600}
                value={healthInterval}
                onChange={(e) => setHealthInterval(parseInt(e.target.value) || 60)}
              />
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={loading}>
            {loading ? "Adding..." : "Add Server"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Server Card Component
function ServerCard({
  server,
  onTest,
  onRemove,
  onViewTools,
}: {
  server: RemoteMcpServer;
  onTest: (id: string) => void;
  onRemove: (id: string) => void;
  onViewTools: (id: string) => void;
}) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div>
            <CardTitle className="text-lg">{server.name}</CardTitle>
            {server.description && (
              <CardDescription>{server.description}</CardDescription>
            )}
          </div>
          <StatusBadge status={server.status} />
        </div>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          <div className="text-sm">
            <span className="text-muted-foreground">Endpoint: </span>
            <code className="text-xs bg-muted px-1 py-0.5 rounded">
              {server.endpoint}
            </code>
          </div>

          <div className="text-sm flex gap-4">
            <span>
              <span className="text-muted-foreground">Auth: </span>
              {server.auth_type === "none" ? "None" : server.auth_type}
            </span>
            {server.latency_ms !== null && (
              <span>
                <span className="text-muted-foreground">Latency: </span>
                {server.latency_ms}ms
              </span>
            )}
          </div>

          {server.last_health_check && (
            <div className="text-xs text-muted-foreground">
              Last checked: {new Date(server.last_health_check).toLocaleString()}
            </div>
          )}

          <div className="flex gap-2 pt-2">
            <Button size="sm" variant="outline" onClick={() => onTest(server.id)}>
              Test Connection
            </Button>
            <Button size="sm" variant="outline" onClick={() => onViewTools(server.id)}>
              View Tools
            </Button>
            <Button
              size="sm"
              variant="destructive"
              onClick={() => onRemove(server.id)}
            >
              Remove
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// Tools Dialog
function ToolsDialog({
  open,
  onOpenChange,
  serverId,
  serverName,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  serverId: string;
  serverName: string;
}) {
  const [tools, setTools] = useState<McpTool[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open && serverId) {
      loadTools();
    }
  }, [open, serverId]);

  const loadTools = async () => {
    setLoading(true);
    setError(null);

    try {
      const result = await invoke<McpTool[]>("list_remote_mcp_tools", { id: serverId });
      setTools(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[80vh]">
        <DialogHeader>
          <DialogTitle>Tools - {serverName}</DialogTitle>
          <DialogDescription>
            Available tools from this MCP server
          </DialogDescription>
        </DialogHeader>

        <ScrollArea className="h-[400px] pr-4">
          {loading && <div className="text-center py-8">Loading tools...</div>}

          {error && (
            <div className="text-red-500 bg-red-50 dark:bg-red-900/20 p-3 rounded">
              {error}
            </div>
          )}

          {!loading && !error && tools.length === 0 && (
            <div className="text-center py-8 text-muted-foreground">
              No tools available from this server
            </div>
          )}

          {!loading && !error && tools.length > 0 && (
            <div className="space-y-3">
              {tools.map((tool) => (
                <Card key={tool.name}>
                  <CardHeader className="py-3">
                    <CardTitle className="text-base font-mono">{tool.name}</CardTitle>
                    {tool.description && (
                      <CardDescription>{tool.description}</CardDescription>
                    )}
                  </CardHeader>
                  <CardContent className="py-2">
                    <details className="text-xs">
                      <summary className="cursor-pointer text-muted-foreground">
                        Input Schema
                      </summary>
                      <pre className="mt-2 p-2 bg-muted rounded overflow-x-auto">
                        {JSON.stringify(tool.input_schema, null, 2)}
                      </pre>
                    </details>
                  </CardContent>
                </Card>
              ))}
            </div>
          )}
        </ScrollArea>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Close
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Main Component
export function RemoteMcpManager() {
  const [servers, setServers] = useState<RemoteMcpServer[]>([]);
  const [loading, setLoading] = useState(true);
  const [addDialogOpen, setAddDialogOpen] = useState(false);
  const [toolsDialogOpen, setToolsDialogOpen] = useState(false);
  const [selectedServer, setSelectedServer] = useState<RemoteMcpServer | null>(null);
  const [testingId, setTestingId] = useState<string | null>(null);

  const loadServers = useCallback(async () => {
    try {
      const result = await invoke<RemoteMcpServer[]>("list_remote_mcp_servers");
      setServers(result);
    } catch (e) {
      console.error("Failed to load servers:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadServers();
  }, [loadServers]);

  const handleTest = async (id: string) => {
    setTestingId(id);
    try {
      await invoke<ServerHealth>("test_remote_mcp_connection", { id });
      await loadServers(); // Refresh to get updated status
    } catch (e) {
      console.error("Connection test failed:", e);
    } finally {
      setTestingId(null);
    }
  };

  const handleRemove = async (id: string) => {
    if (!confirm("Are you sure you want to remove this server?")) return;

    try {
      await invoke("remove_remote_mcp_server", { id });
      await loadServers();
    } catch (e) {
      console.error("Failed to remove server:", e);
    }
  };

  const handleViewTools = (id: string) => {
    const server = servers.find((s) => s.id === id);
    if (server) {
      setSelectedServer(server);
      setToolsDialogOpen(true);
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Remote MCP Servers</h2>
          <p className="text-muted-foreground">
            Connect to MCP servers on your VPS using Streamable HTTP
          </p>
        </div>
        <Button onClick={() => setAddDialogOpen(true)}>Add Server</Button>
      </div>

      {loading ? (
        <div className="text-center py-8">Loading servers...</div>
      ) : servers.length === 0 ? (
        <Card>
          <CardContent className="py-8 text-center">
            <p className="text-muted-foreground mb-4">
              No remote MCP servers configured yet
            </p>
            <Button onClick={() => setAddDialogOpen(true)}>
              Add Your First Server
            </Button>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {servers.map((server) => (
            <ServerCard
              key={server.id}
              server={server}
              onTest={handleTest}
              onRemove={handleRemove}
              onViewTools={handleViewTools}
            />
          ))}
        </div>
      )}

      <AddServerDialog
        open={addDialogOpen}
        onOpenChange={setAddDialogOpen}
        onAdd={loadServers}
      />

      {selectedServer && (
        <ToolsDialog
          open={toolsDialogOpen}
          onOpenChange={setToolsDialogOpen}
          serverId={selectedServer.id}
          serverName={selectedServer.name}
        />
      )}
    </div>
  );
}

export default RemoteMcpManager;
