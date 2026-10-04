/** `getDocument` failed: its message as-is. The editor below stays read-only. */
export function LoadError({ message }: { message: string }) {
  return (
    <div className="mx-auto w-full max-w-[680px] px-10 pt-6">
      <p role="alert" className="rounded-lg border border-destructive/30 bg-destructive/5 px-3.5 py-2.5 text-[13px] text-destructive">
        {message}
      </p>
    </div>
  );
}
