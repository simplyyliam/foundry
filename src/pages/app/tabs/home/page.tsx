import { usePageHeader } from "@/shared";

export default function Home() {
   usePageHeader({ title: "Home", subtitle: "Your workspace at a glance" });
  return (
    <div className="flex items-center justify-center h-full w-full bg-background rounded-[9px]">
      Foundry Home
    </div>
  )
}
