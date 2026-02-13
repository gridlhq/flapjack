import { useState, useCallback } from 'react';
import { useAuth } from '@/hooks/useAuth';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';

interface ConnectionDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ConnectionDialog({ open, onOpenChange }: ConnectionDialogProps) {
  const { apiKey, appId, setApiKey, setAppId, clearAuth } = useAuth();
  const [keyInput, setKeyInput] = useState(apiKey || '');
  const [appIdInput, setAppIdInput] = useState(appId || 'flapjack');

  const handleSave = useCallback(() => {
    if (keyInput.trim()) {
      setApiKey(keyInput.trim());
    } else {
      clearAuth();
    }
    if (appIdInput.trim()) {
      setAppId(appIdInput.trim());
    }
    onOpenChange(false);
    window.location.reload();
  }, [keyInput, appIdInput, setApiKey, setAppId, clearAuth, onOpenChange]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle>Connection Settings</DialogTitle>
          <DialogDescription>
            Configure your Flapjack server connection
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4">
          <div className="space-y-2">
            <Label>Admin API Key</Label>
            <Input
              type="password"
              value={keyInput}
              onChange={(e) => setKeyInput(e.target.value)}
              placeholder="Enter your admin API key"
            />
            <p className="text-xs text-muted-foreground">
              Leave empty if server runs without authentication
            </p>
          </div>

          <div className="space-y-2">
            <Label>Application ID</Label>
            <Input
              value={appIdInput}
              onChange={(e) => setAppIdInput(e.target.value)}
              placeholder="flapjack"
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave}>
            Save & Reconnect
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
