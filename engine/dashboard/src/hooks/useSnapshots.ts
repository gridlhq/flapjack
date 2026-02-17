import { useQuery, useMutation } from '@tanstack/react-query';
import api from '@/lib/api';
import { useToast } from '@/hooks/use-toast';

export interface S3Snapshot {
  name: string;
  size: number;
  lastModified: string;
}

/**
 * Export an index as a tar.gz download.
 */
export function useExportIndex() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (indexName: string) => {
      const response = await api.get(`/1/indexes/${indexName}/export`, {
        responseType: 'blob',
      });
      // Trigger browser download
      const blob = new Blob([response.data], { type: 'application/gzip' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${indexName}.tar.gz`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    },
    onSuccess: (_data, indexName) => {
      toast({ title: 'Export complete', description: `Index "${indexName}" downloaded as tar.gz.` });
    },
    onError: (error: Error, indexName) => {
      toast({ variant: 'destructive', title: 'Export failed', description: `Failed to export "${indexName}": ${error.message}` });
    },
  });
}

/**
 * Import a tar.gz file into an index.
 */
export function useImportIndex() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: async ({ indexName, file }: { indexName: string; file: File }) => {
      const { data } = await api.post(`/1/indexes/${indexName}/import`, file, {
        headers: { 'Content-Type': 'application/gzip' },
      });
      return data;
    },
    onSuccess: (_data, { indexName }) => {
      toast({ title: 'Import started', description: `Importing data into "${indexName}".` });
    },
    onError: (error: Error, { indexName }) => {
      toast({ variant: 'destructive', title: 'Import failed', description: `Failed to import into "${indexName}": ${error.message}` });
    },
  });
}

/**
 * Backup an index to S3.
 */
export function useSnapshotToS3() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (indexName: string) => {
      const { data } = await api.post(`/1/indexes/${indexName}/snapshot`);
      return data;
    },
    onSuccess: (_data, indexName) => {
      toast({ title: 'Backup started', description: `Backing up "${indexName}" to S3.` });
    },
    onError: (error: Error, indexName) => {
      toast({ variant: 'destructive', title: 'Backup failed', description: `Failed to backup "${indexName}": ${error.message}` });
    },
  });
}

/**
 * Restore an index from S3.
 */
export function useRestoreFromS3() {
  const { toast } = useToast();
  return useMutation({
    mutationFn: async (indexName: string) => {
      const { data } = await api.post(`/1/indexes/${indexName}/restore`);
      return data;
    },
    onSuccess: (_data, indexName) => {
      toast({ title: 'Restore started', description: `Restoring "${indexName}" from S3.` });
    },
    onError: (error: Error, indexName) => {
      toast({ variant: 'destructive', title: 'Restore failed', description: `Failed to restore "${indexName}": ${error.message}` });
    },
  });
}

/**
 * List S3 snapshots for an index.
 */
export function useListSnapshots(indexName: string) {
  return useQuery<S3Snapshot[]>({
    queryKey: ['snapshots', indexName],
    queryFn: async () => {
      const { data } = await api.get(`/1/indexes/${indexName}/snapshots`);
      return data.snapshots || data.results || data || [];
    },
    enabled: !!indexName,
    retry: false, // Don't retry — S3 may not be configured
    staleTime: 60000,
  });
}
