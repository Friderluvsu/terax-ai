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

type Props = {
  path: string | null;
  onAnswer: (run: boolean) => void;
};

export function ScriptQuarantineDialog({ path, onAnswer }: Props) {
  return (
    <AlertDialog
      open={path !== null}
      onOpenChange={(open) => {
        if (!open) onAnswer(false);
      }}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Run downloaded script?</AlertDialogTitle>
          <AlertDialogDescription>
            The file was downloaded from the internet. Run it?
            {path ? ` ${path}` : ""}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => onAnswer(false)}>
            Cancel
          </AlertDialogCancel>
          <AlertDialogAction onClick={() => onAnswer(true)}>
            Run
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
