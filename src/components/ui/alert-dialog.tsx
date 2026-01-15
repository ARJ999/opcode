import * as React from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

interface AlertDialogProps {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  children?: React.ReactNode;
}

const AlertDialog = ({ open, onOpenChange, children }: AlertDialogProps) => {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {children}
    </Dialog>
  );
};

const AlertDialogContent = ({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) => {
  return <DialogContent className={className}>{children}</DialogContent>;
};

const AlertDialogHeader = ({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) => {
  return <DialogHeader className={className}>{children}</DialogHeader>;
};

const AlertDialogTitle = ({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) => {
  return <DialogTitle className={className}>{children}</DialogTitle>;
};

const AlertDialogDescription = ({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) => {
  return <DialogDescription className={className}>{children}</DialogDescription>;
};

const AlertDialogFooter = ({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) => {
  return <DialogFooter className={className}>{children}</DialogFooter>;
};

const AlertDialogAction = ({
  children,
  onClick,
  className,
}: {
  children?: React.ReactNode;
  onClick?: () => void;
  className?: string;
}) => {
  return (
    <Button onClick={onClick} className={className}>
      {children}
    </Button>
  );
};

const AlertDialogCancel = ({
  children,
  className,
}: {
  children?: React.ReactNode;
  className?: string;
}) => {
  return (
    <Button variant="outline" className={className}>
      {children || "Cancel"}
    </Button>
  );
};

export {
  AlertDialog,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogAction,
  AlertDialogCancel,
};
