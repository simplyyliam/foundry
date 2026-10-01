export interface DocumentProps {
  /** The project whose document this is. */
  projectId: string
}

/** Shell for a project's markdown document. The editor goes inside. */
export function Document({ projectId }: DocumentProps) {
  return (
    <div data-project-id={projectId} className="flex h-full w-full flex-col bg-background">
      {/* Rich-text markdown editor from @/features/document goes here. */}
    </div>
  )
}
