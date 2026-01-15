/**
 * Task Manager
 *
 * Opcode 2.0 - Monitor and manage parallel background tasks.
 */

import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Progress } from "@/components/ui/progress";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@/components/ui/tabs";
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
interface TaskProgress {
  current: number;
  total: number | null;
  percentage: number | null;
  message: string;
  details: string | null;
}

interface TaskInfo {
  id: string;
  kind: string;
  name: string;
  description: string | null;
  status: string;
  priority: string;
  progress: TaskProgress;
  background: boolean;
  cancellable: boolean;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  duration_ms: number | null;
}

interface TaskCount {
  total: number;
  active: number;
  completed: number;
  failed: number;
  cancelled: number;
}

// Status icons and colors
const STATUS_CONFIG: Record<string, { icon: string; color: string; variant: "default" | "secondary" | "destructive" | "outline" }> = {
  pending: { icon: "⏳", color: "text-yellow-500", variant: "outline" },
  running: { icon: "🔄", color: "text-blue-500", variant: "default" },
  completed: { icon: "✓", color: "text-green-500", variant: "secondary" },
  failed: { icon: "✗", color: "text-red-500", variant: "destructive" },
  cancelled: { icon: "⊘", color: "text-gray-500", variant: "outline" },
  paused: { icon: "⏸", color: "text-orange-500", variant: "outline" },
};

// Kind labels
const KIND_LABELS: Record<string, string> = {
  agentexecution: "Agent",
  skillexecution: "Skill",
  shell: "Shell",
  fileoperation: "File",
  mcptoolcall: "MCP",
  checkpoint: "Checkpoint",
  sync: "Sync",
  async: "Async",
};

// Format duration
function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

// Status Badge Component
function StatusBadge({ status }: { status: string }) {
  const config = STATUS_CONFIG[status] || STATUS_CONFIG.pending;
  return (
    <Badge variant={config.variant} className="gap-1">
      <span>{config.icon}</span>
      {status}
    </Badge>
  );
}

// Task Card Component
function TaskCard({
  task,
  onCancel,
}: {
  task: TaskInfo;
  onCancel: (id: string) => void;
}) {
  const isActive = task.status === "pending" || task.status === "running";
  const kindLabel = KIND_LABELS[task.kind] || task.kind;

  return (
    <Card className={!isActive ? "opacity-80" : ""}>
      <CardHeader className="pb-2">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Badge variant="outline">{kindLabel}</Badge>
            <CardTitle className="text-base">{task.name}</CardTitle>
          </div>
          <StatusBadge status={task.status} />
        </div>
        {task.description && (
          <CardDescription className="text-sm">
            {task.description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {/* Progress */}
          {task.status === "running" && (
            <div className="space-y-1">
              <div className="flex justify-between text-xs text-muted-foreground">
                <span>{task.progress.message}</span>
                {task.progress.percentage !== null && (
                  <span>{task.progress.percentage.toFixed(0)}%</span>
                )}
              </div>
              {task.progress.percentage !== null ? (
                <Progress value={task.progress.percentage} />
              ) : (
                <Progress className="animate-pulse" />
              )}
            </div>
          )}

          {/* Meta info */}
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <div className="flex gap-2">
              {task.background && <Badge variant="outline">Background</Badge>}
              {task.duration_ms && (
                <span>Duration: {formatDuration(task.duration_ms)}</span>
              )}
            </div>
            <div className="flex gap-2">
              {isActive && task.cancellable && (
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => onCancel(task.id)}
                >
                  Cancel
                </Button>
              )}
            </div>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

// Main Component
export function TaskManager() {
  const [tasks, setTasks] = useState<TaskInfo[]>([]);
  const [counts, setCounts] = useState<TaskCount | null>(null);
  const [loading, setLoading] = useState(true);
  const [activeTab, setActiveTab] = useState("active");
  const [cancelId, setCancelId] = useState<string | null>(null);

  const loadTasks = useCallback(async () => {
    try {
      const [allTasks, taskCounts] = await Promise.all([
        invoke<TaskInfo[]>("list_tasks"),
        invoke<TaskCount>("get_task_count"),
      ]);
      setTasks(allTasks);
      setCounts(taskCounts);
    } catch (e) {
      console.error("Failed to load tasks:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadTasks();

    // Set up event listeners
    const unlisteners: Promise<UnlistenFn>[] = [];

    const events = [
      "task:created",
      "task:started",
      "task:progress",
      "task:completed",
      "task:cancelled",
      "task:failed",
    ];

    for (const event of events) {
      unlisteners.push(
        listen(event, () => {
          loadTasks();
        })
      );
    }

    // Auto-refresh every 5 seconds
    const interval = setInterval(loadTasks, 5000);

    return () => {
      clearInterval(interval);
      Promise.all(unlisteners).then((fns) => fns.forEach((fn) => fn()));
    };
  }, [loadTasks]);

  const handleCancel = async () => {
    if (!cancelId) return;

    try {
      await invoke("cancel_task", { id: cancelId });
      setCancelId(null);
      await loadTasks();
    } catch (e) {
      console.error("Failed to cancel task:", e);
    }
  };

  const handleClearCompleted = async () => {
    try {
      await invoke("clear_completed_tasks");
      await loadTasks();
    } catch (e) {
      console.error("Failed to clear completed tasks:", e);
    }
  };

  const filteredTasks = tasks.filter((task) => {
    switch (activeTab) {
      case "active":
        return task.status === "pending" || task.status === "running";
      case "background":
        return (
          task.background &&
          (task.status === "pending" || task.status === "running")
        );
      case "completed":
        return task.status === "completed";
      case "failed":
        return task.status === "failed" || task.status === "cancelled";
      default:
        return true;
    }
  });

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">Tasks</h2>
          <p className="text-muted-foreground">
            Monitor and manage background operations
          </p>
        </div>
        {counts && counts.completed + counts.failed + counts.cancelled > 0 && (
          <Button variant="outline" onClick={handleClearCompleted}>
            Clear Completed
          </Button>
        )}
      </div>

      {/* Stats */}
      {counts && (
        <div className="grid grid-cols-5 gap-4">
          <Card>
            <CardContent className="py-3 text-center">
              <div className="text-2xl font-bold">{counts.total}</div>
              <div className="text-xs text-muted-foreground">Total</div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-3 text-center">
              <div className="text-2xl font-bold text-blue-500">
                {counts.active}
              </div>
              <div className="text-xs text-muted-foreground">Active</div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-3 text-center">
              <div className="text-2xl font-bold text-green-500">
                {counts.completed}
              </div>
              <div className="text-xs text-muted-foreground">Completed</div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-3 text-center">
              <div className="text-2xl font-bold text-red-500">
                {counts.failed}
              </div>
              <div className="text-xs text-muted-foreground">Failed</div>
            </CardContent>
          </Card>
          <Card>
            <CardContent className="py-3 text-center">
              <div className="text-2xl font-bold text-gray-500">
                {counts.cancelled}
              </div>
              <div className="text-xs text-muted-foreground">Cancelled</div>
            </CardContent>
          </Card>
        </div>
      )}

      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList>
          <TabsTrigger value="active">
            Active ({counts?.active || 0})
          </TabsTrigger>
          <TabsTrigger value="background">Background</TabsTrigger>
          <TabsTrigger value="completed">
            Completed ({counts?.completed || 0})
          </TabsTrigger>
          <TabsTrigger value="failed">
            Failed ({(counts?.failed || 0) + (counts?.cancelled || 0)})
          </TabsTrigger>
        </TabsList>

        <TabsContent value={activeTab} className="mt-4">
          {loading ? (
            <div className="text-center py-8">Loading tasks...</div>
          ) : filteredTasks.length === 0 ? (
            <Card>
              <CardContent className="py-8 text-center">
                <p className="text-muted-foreground">
                  No {activeTab} tasks
                </p>
              </CardContent>
            </Card>
          ) : (
            <ScrollArea className="h-[500px]">
              <div className="space-y-3 pr-4">
                {filteredTasks.map((task) => (
                  <TaskCard
                    key={task.id}
                    task={task}
                    onCancel={(id) => setCancelId(id)}
                  />
                ))}
              </div>
            </ScrollArea>
          )}
        </TabsContent>
      </Tabs>

      {/* Cancel Confirmation */}
      <AlertDialog open={!!cancelId} onOpenChange={() => setCancelId(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Cancel Task</AlertDialogTitle>
            <AlertDialogDescription>
              Are you sure you want to cancel this task? This may leave operations in an incomplete state.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Keep Running</AlertDialogCancel>
            <AlertDialogAction onClick={handleCancel}>
              Cancel Task
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}

export default TaskManager;
