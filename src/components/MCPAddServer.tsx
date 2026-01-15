import React, { useState } from "react";
import { Plus, Globe, Key, Shield, Loader2, Info, Eye, EyeOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { SelectComponent } from "@/components/ui/select";
import { Card } from "@/components/ui/card";
import { api } from "@/lib/api";
import { useTrackEvent } from "@/hooks";

interface MCPAddServerProps {
  /**
   * Callback when a server is successfully added
   */
  onServerAdded: () => void;
  /**
   * Callback for error messages
   */
  onError: (message: string) => void;
}

type AuthType = "none" | "bearer" | "api_key";

/**
 * Component for adding new MCP servers via Streamable HTTP
 * Supports HTTPS endpoints with Bearer token or API key authentication
 *
 * Opcode 2.0 - MCP Specification 2025-11-25 compliant
 */
export const MCPAddServer: React.FC<MCPAddServerProps> = ({
  onServerAdded,
  onError,
}) => {
  const [saving, setSaving] = useState(false);
  const [showSecret, setShowSecret] = useState(false);

  // Analytics tracking
  const trackEvent = useTrackEvent();

  // Server configuration state
  const [name, setName] = useState("");
  const [endpoint, setEndpoint] = useState("");
  const [authType, setAuthType] = useState<AuthType>("bearer");
  const [authToken, setAuthToken] = useState("");
  const [apiKeyHeader, setApiKeyHeader] = useState("X-API-Key");

  /**
   * Validates HTTPS URL
   */
  const isValidHttpsUrl = (url: string): boolean => {
    try {
      const parsed = new URL(url);
      return parsed.protocol === "https:";
    } catch {
      return false;
    }
  };

  /**
   * Validates and adds the remote MCP server
   */
  const handleAddServer = async () => {
    // Validate name
    if (!name.trim()) {
      onError("Server name is required");
      return;
    }

    // Validate endpoint
    if (!endpoint.trim()) {
      onError("HTTPS endpoint is required");
      return;
    }

    if (!isValidHttpsUrl(endpoint)) {
      onError("Please enter a valid HTTPS URL (e.g., https://your-server.com/mcp)");
      return;
    }

    // Validate authentication
    if (authType !== "none" && !authToken.trim()) {
      onError(`${authType === "bearer" ? "Bearer token" : "API key"} is required`);
      return;
    }

    try {
      setSaving(true);

      // Build auth configuration
      let authConfig: { type: string; token?: string; header_name?: string } | undefined;

      if (authType === "bearer") {
        authConfig = {
          type: "bearer",
          token: authToken.trim(),
        };
      } else if (authType === "api_key") {
        authConfig = {
          type: "api_key",
          token: authToken.trim(),
          header_name: apiKeyHeader.trim() || "X-API-Key",
        };
      }

      // Call the remote MCP add API
      const result = await api.addRemoteMcpServer(
        name.trim(),
        endpoint.trim(),
        authConfig
      );

      if (result.success) {
        // Track server added
        trackEvent.mcpServerAdded({
          server_type: "streamable_http",
          configuration_method: "manual"
        });

        // Reset form
        setName("");
        setEndpoint("");
        setAuthType("bearer");
        setAuthToken("");
        setApiKeyHeader("X-API-Key");
        onServerAdded();
      } else {
        onError(result.message || "Failed to add server");
      }
    } catch (error) {
      onError("Failed to add server");
      console.error("Failed to add remote MCP server:", error);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="p-6 space-y-6">
      <div>
        <h3 className="text-base font-semibold flex items-center gap-2">
          <Globe className="h-5 w-5 text-emerald-500" />
          Add Remote MCP Server
        </h3>
        <p className="text-sm text-muted-foreground mt-1">
          Connect to your MCP server via Streamable HTTP (HTTPS)
        </p>
      </div>

      <Card className="p-6 space-y-6">
        <div className="space-y-4">
          {/* Server Name */}
          <div className="space-y-2">
            <Label htmlFor="server-name">Server Name</Label>
            <Input
              id="server-name"
              placeholder="my-mcp-server"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <p className="text-xs text-muted-foreground">
              A unique name to identify this server
            </p>
          </div>

          {/* HTTPS Endpoint */}
          <div className="space-y-2">
            <Label htmlFor="endpoint">HTTPS Endpoint</Label>
            <Input
              id="endpoint"
              placeholder="https://your-vps.com/mcp"
              value={endpoint}
              onChange={(e) => setEndpoint(e.target.value)}
              className="font-mono"
            />
            <p className="text-xs text-muted-foreground">
              The Streamable HTTP endpoint URL of your MCP server
            </p>
          </div>

          {/* Authentication Type */}
          <div className="space-y-2">
            <Label htmlFor="auth-type" className="flex items-center gap-2">
              <Shield className="h-4 w-4 text-primary" />
              Authentication
            </Label>
            <SelectComponent
              value={authType}
              onValueChange={(value: string) => setAuthType(value as AuthType)}
              options={[
                { value: "bearer", label: "Bearer Token (Recommended)" },
                { value: "api_key", label: "API Key" },
                { value: "none", label: "No Authentication" },
              ]}
            />
          </div>

          {/* Bearer Token / API Key Input */}
          {authType !== "none" && (
            <div className="space-y-4 pt-2 border-t border-border">
              {authType === "api_key" && (
                <div className="space-y-2">
                  <Label htmlFor="api-key-header">API Key Header Name</Label>
                  <Input
                    id="api-key-header"
                    placeholder="X-API-Key"
                    value={apiKeyHeader}
                    onChange={(e) => setApiKeyHeader(e.target.value)}
                    className="font-mono"
                  />
                  <p className="text-xs text-muted-foreground">
                    The HTTP header name for your API key (default: X-API-Key)
                  </p>
                </div>
              )}

              <div className="space-y-2">
                <Label htmlFor="auth-token" className="flex items-center gap-2">
                  <Key className="h-4 w-4" />
                  {authType === "bearer" ? "Bearer Token" : "API Key"}
                </Label>
                <div className="relative">
                  <Input
                    id="auth-token"
                    type={showSecret ? "text" : "password"}
                    placeholder={authType === "bearer" ? "your-bearer-token" : "your-api-key"}
                    value={authToken}
                    onChange={(e) => setAuthToken(e.target.value)}
                    className="font-mono pr-10"
                  />
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    className="absolute right-0 top-0 h-full w-10 hover:bg-transparent"
                    onClick={() => setShowSecret(!showSecret)}
                  >
                    {showSecret ? (
                      <EyeOff className="h-4 w-4 text-muted-foreground" />
                    ) : (
                      <Eye className="h-4 w-4 text-muted-foreground" />
                    )}
                  </Button>
                </div>
                <p className="text-xs text-muted-foreground">
                  {authType === "bearer"
                    ? "Your server's Bearer token for Authorization header"
                    : "Your API key value"}
                </p>
              </div>
            </div>
          )}
        </div>

        <div className="pt-2">
          <Button
            onClick={handleAddServer}
            disabled={saving}
            className="w-full gap-2 bg-primary hover:bg-primary/90"
          >
            {saving ? (
              <>
                <Loader2 className="h-4 w-4 animate-spin" />
                Adding Server...
              </>
            ) : (
              <>
                <Plus className="h-4 w-4" />
                Add Remote Server
              </>
            )}
          </Button>
        </div>
      </Card>

      {/* Info Card */}
      <Card className="p-4 bg-muted/30">
        <div className="space-y-3">
          <div className="flex items-center gap-2 text-sm font-medium">
            <Info className="h-4 w-4 text-primary" />
            <span>Streamable HTTP (MCP 2025-11-25)</span>
          </div>
          <div className="space-y-2 text-xs text-muted-foreground">
            <p>
              Connect to your remote MCP servers hosted on VPS via secure HTTPS connections.
              Streamable HTTP is the modern MCP transport standard that replaces SSE.
            </p>
            <div className="font-mono bg-background p-2 rounded space-y-1">
              <p>• Example: https://your-server.com/mcp</p>
              <p>• Requires HTTPS (TLS/SSL encrypted)</p>
              <p>• Supports Bearer token or API key auth</p>
            </div>
          </div>
        </div>
      </Card>
    </div>
  );
};
