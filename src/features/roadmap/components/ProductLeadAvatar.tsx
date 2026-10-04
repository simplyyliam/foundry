import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { DEFAULT_AVATAR, useProductLeadAvatar } from "../hooks/useProductLeadAvatar";

/** The Product Lead's avatar; the default one when there's none or it fails to load. */
export function ProductLeadAvatar({ projectId }: { projectId: string }) {
  const src = useProductLeadAvatar(projectId);
  return (
    <Avatar size="sm" className="size-5">
      <AvatarImage src={src} alt="" />
      <AvatarFallback>
        <img src={DEFAULT_AVATAR} alt="" className="size-full rounded-full object-cover" />
      </AvatarFallback>
    </Avatar>
  );
}
