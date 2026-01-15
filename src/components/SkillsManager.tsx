/**
 * Skills Manager
 *
 * Opcode 2.0 - Unified skills management interface.
 * Manage slash commands, hooks, workflows, and templates.
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
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
import { Badge } from "@/components/ui/badge";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Switch } from "@/components/ui/switch";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

// Types
interface SkillInfo {
  id: string;
  kind: string;
  name: string;
  description: string;
  visibility: string;
  enabled: boolean;
  source: string;
  project_path: string | null;
  created_at: string;
  updated_at: string;
}

interface SlashCommand {
  id: string;
  name: string;
  description: string;
  help: string | null;
  requires_args: boolean;
  examples: string[];
}

// Skill type icons
const SKILL_ICONS: Record<string, string> = {
  slash_command: "/",
  hook: "⚡",
  workflow: "🔄",
  template: "📝",
  agent: "🤖",
};

// Kind badge component
function KindBadge({ kind }: { kind: string }) {
  const variants: Record<string, "default" | "secondary" | "outline"> = {
    slash_command: "default",
    hook: "secondary",
    workflow: "outline",
    template: "outline",
    agent: "default",
  };

  return (
    <Badge variant={variants[kind] || "outline"} className="gap-1">
      <span>{SKILL_ICONS[kind] || "?"}</span>
      {kind.replace("_", " ")}
    </Badge>
  );
}

// Create Slash Command Dialog
function CreateSlashCommandDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [prompt, setPrompt] = useState("");
  const [help, setHelp] = useState("");
  const [visibility, setVisibility] = useState("global");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!name.trim() || !prompt.trim()) {
      setError("Name and prompt are required");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await invoke("create_slash_command", {
        request: {
          name: name.trim().replace(/^\//, ""), // Remove leading slash if present
          description: description.trim(),
          prompt: prompt.trim(),
          help: help.trim() || null,
          visibility,
        },
      });

      // Reset form
      setName("");
      setDescription("");
      setPrompt("");
      setHelp("");
      setVisibility("global");

      onCreated();
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
          <DialogTitle>Create Slash Command</DialogTitle>
          <DialogDescription>
            Create a new slash command that expands into a prompt template.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {error && (
            <div className="text-sm text-red-500 bg-red-50 dark:bg-red-900/20 p-2 rounded">
              {error}
            </div>
          )}

          <div className="space-y-2">
            <Label htmlFor="name">Command Name</Label>
            <div className="flex items-center gap-2">
              <span className="text-muted-foreground">/</span>
              <Input
                id="name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="my-command"
                className="flex-1"
              />
            </div>
            <p className="text-xs text-muted-foreground">
              Users will type /{name || "command"} to use this
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="description">Description</Label>
            <Input
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What this command does..."
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="prompt">Prompt Template</Label>
            <Textarea
              id="prompt"
              value={prompt}
              onChange={(e) => setPrompt(e.target.value)}
              placeholder="The prompt that will be sent to Claude. Use $ARGUMENTS to include user input."
              rows={4}
            />
            <p className="text-xs text-muted-foreground">
              Use <code className="bg-muted px-1">$ARGUMENTS</code> to include any text the user types after the command
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="help">Help Text (optional)</Label>
            <Textarea
              id="help"
              value={help}
              onChange={(e) => setHelp(e.target.value)}
              placeholder="Extended help and usage examples..."
              rows={2}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="visibility">Visibility</Label>
            <Select value={visibility} onValueChange={setVisibility}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">Global (all projects)</SelectItem>
                <SelectItem value="project">Current Project Only</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={loading}>
            {loading ? "Creating..." : "Create Command"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Create Hook Dialog
function CreateHookDialog({
  open,
  onOpenChange,
  onCreated,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCreated: () => void;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [trigger, setTrigger] = useState("pre_tool");
  const [command, setCommand] = useState("");
  const [timeout, setTimeout] = useState("30");
  const [canBlock, setCanBlock] = useState(false);
  const [visibility, setVisibility] = useState("global");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async () => {
    if (!name.trim() || !command.trim()) {
      setError("Name and command are required");
      return;
    }

    setLoading(true);
    setError(null);

    try {
      await invoke("create_hook", {
        request: {
          name: name.trim(),
          description: description.trim(),
          trigger,
          command: command.trim(),
          timeout_secs: parseInt(timeout) || 30,
          can_block: canBlock,
          visibility,
        },
      });

      // Reset form
      setName("");
      setDescription("");
      setTrigger("pre_tool");
      setCommand("");
      setTimeout("30");
      setCanBlock(false);
      setVisibility("global");

      onCreated();
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
          <DialogTitle>Create Hook</DialogTitle>
          <DialogDescription>
            Create a hook that runs a command at specific trigger points.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          {error && (
            <div className="text-sm text-red-500 bg-red-50 dark:bg-red-900/20 p-2 rounded">
              {error}
            </div>
          )}

          <div className="space-y-2">
            <Label htmlFor="hookName">Hook Name</Label>
            <Input
              id="hookName"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="my-hook"
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="hookDescription">Description</Label>
            <Input
              id="hookDescription"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="What this hook does..."
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="trigger">Trigger</Label>
            <Select value={trigger} onValueChange={setTrigger}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="pre_tool">Before Tool Execution</SelectItem>
                <SelectItem value="post_tool">After Tool Execution</SelectItem>
                <SelectItem value="session_start">Session Start</SelectItem>
                <SelectItem value="session_end">Session End</SelectItem>
                <SelectItem value="checkpoint_create">Checkpoint Create</SelectItem>
                <SelectItem value="on_error">On Error</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label htmlFor="command">Command</Label>
            <Textarea
              id="command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="The shell command to run..."
              rows={2}
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="timeout">Timeout (seconds)</Label>
              <Input
                id="timeout"
                type="number"
                min="1"
                max="300"
                value={timeout}
                onChange={(e) => setTimeout(e.target.value)}
              />
            </div>

            <div className="flex items-center justify-between pt-6">
              <div className="space-y-0.5">
                <Label>Can Block</Label>
                <p className="text-xs text-muted-foreground">
                  Can modify execution
                </p>
              </div>
              <Switch checked={canBlock} onCheckedChange={setCanBlock} />
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="hookVisibility">Visibility</Label>
            <Select value={visibility} onValueChange={setVisibility}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="global">Global (all projects)</SelectItem>
                <SelectItem value="project">Current Project Only</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={loading}>
            {loading ? "Creating..." : "Create Hook"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// Skill Card Component
function SkillCard({
  skill,
  onEdit,
  onDelete,
  onToggle,
}: {
  skill: SkillInfo;
  onEdit: (id: string) => void;
  onDelete: (id: string) => void;
  onToggle: (id: string, enabled: boolean) => void;
}) {
  return (
    <Card className={!skill.enabled ? "opacity-60" : ""}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <KindBadge kind={skill.kind} />
            <CardTitle className="text-base">{skill.name}</CardTitle>
          </div>
          <Switch
            checked={skill.enabled}
            onCheckedChange={(checked) => onToggle(skill.id, checked)}
          />
        </div>
        {skill.description && (
          <CardDescription className="text-sm">
            {skill.description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        <div className="flex items-center justify-between">
          <div className="flex gap-2 text-xs text-muted-foreground">
            <span>{skill.visibility}</span>
            <span>•</span>
            <span>{skill.source}</span>
          </div>
          <div className="flex gap-2">
            <Button size="sm" variant="outline" onClick={() => onEdit(skill.id)}>
              Edit
            </Button>
            <Button
              size="sm"
              variant="destructive"
              onClick={() => onDelete(skill.id)}
            >
              Delete
            </Button>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// Main Component
export function SkillsManager() {
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState("all");
  const [createSlashOpen, setCreateSlashOpen] = useState(false);
  const [createHookOpen, setCreateHookOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [importPath, setImportPath] = useState("");
  const [importing, setImporting] = useState(false);

  const loadSkills = useCallback(async () => {
    try {
      const kind = activeTab === "all" ? undefined : activeTab;
      const result = await invoke<SkillInfo[]>("list_skills", { kind });
      setSkills(result);
    } catch (e) {
      console.error("Failed to load skills:", e);
    } finally {
      setLoading(false);
    }
  }, [activeTab]);

  useEffect(() => {
    loadSkills();
  }, [loadSkills]);

  const handleToggle = async (id: string, enabled: boolean) => {
    try {
      await invoke("update_skill", { id, enabled });
      await loadSkills();
    } catch (e) {
      console.error("Failed to toggle skill:", e);
    }
  };

  const handleDelete = async () => {
    if (!deleteId) return;

    try {
      await invoke("delete_skill", { id: deleteId });
      setDeleteId(null);
      await loadSkills();
    } catch (e) {
      console.error("Failed to delete skill:", e);
    }
  };

  const handleImport = async () => {
    if (!importPath.trim()) return;

    setImporting(true);
    try {
      await invoke("import_claude_code_skills", { settingsPath: importPath });
      setImportPath("");
      await loadSkills();
    } catch (e) {
      console.error("Failed to import skills:", e);
    } finally {
      setImporting(false);
    }
  };

  const filteredSkills =
    activeTab === "all"
      ? skills
      : skills.filter((s) => s.kind === activeTab);

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Skills</h2>
          <p className="text-muted-foreground">
            Manage slash commands, hooks, workflows, and more
          </p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" onClick={() => setCreateHookOpen(true)}>
            New Hook
          </Button>
          <Button onClick={() => setCreateSlashOpen(true)}>
            New Slash Command
          </Button>
        </div>
      </div>

      {/* Import from Claude Code */}
      <Card>
        <CardContent className="py-4">
          <div className="flex items-end gap-4">
            <div className="flex-1 space-y-2">
              <Label>Import from Claude Code Settings</Label>
              <Input
                value={importPath}
                onChange={(e) => setImportPath(e.target.value)}
                placeholder="Path to .claude/settings.toml"
              />
            </div>
            <Button
              variant="outline"
              onClick={handleImport}
              disabled={importing || !importPath.trim()}
            >
              {importing ? "Importing..." : "Import"}
            </Button>
          </div>
        </CardContent>
      </Card>

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="all">All ({skills.length})</TabsTrigger>
          <TabsTrigger value="slash_command">
            Slash Commands ({skills.filter((s) => s.kind === "slash_command").length})
          </TabsTrigger>
          <TabsTrigger value="hook">
            Hooks ({skills.filter((s) => s.kind === "hook").length})
          </TabsTrigger>
          <TabsTrigger value="workflow">
            Workflows ({skills.filter((s) => s.kind === "workflow").length})
          </TabsTrigger>
        </TabsList>

        <TabsContent value={activeTab} className="mt-4">
          {loading ? (
            <div className="text-center py-8">Loading skills...</div>
          ) : filteredSkills.length === 0 ? (
            <Card>
              <CardContent className="py-8 text-center">
                <p className="text-muted-foreground mb-4">
                  No {activeTab === "all" ? "skills" : activeTab.replace("_", " ") + "s"} yet
                </p>
                {activeTab === "all" || activeTab === "slash_command" ? (
                  <Button onClick={() => setCreateSlashOpen(true)}>
                    Create Slash Command
                  </Button>
                ) : activeTab === "hook" ? (
                  <Button onClick={() => setCreateHookOpen(true)}>
                    Create Hook
                  </Button>
                ) : null}
              </CardContent>
            </Card>
          ) : (
            <div className="grid gap-4 md:grid-cols-2">
              {filteredSkills.map((skill) => (
                <SkillCard
                  key={skill.id}
                  skill={skill}
                  onEdit={(id) => console.log("Edit:", id)}
                  onDelete={(id) => setDeleteId(id)}
                  onToggle={handleToggle}
                />
              ))}
            </div>
          )}
        </TabsContent>
      </Tabs>

      {/* Create Dialogs */}
      <CreateSlashCommandDialog
        open={createSlashOpen}
        onOpenChange={setCreateSlashOpen}
        onCreated={loadSkills}
      />

      <CreateHookDialog
        open={createHookOpen}
        onOpenChange={setCreateHookOpen}
        onCreated={loadSkills}
      />

      {/* Delete Confirmation */}
      <AlertDialog open={!!deleteId} onOpenChange={() => setDeleteId(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete Skill</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to delete this skill? This action cannot be undone.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction onClick={handleDelete}>Delete</AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

export default SkillsManager;
