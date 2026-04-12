import { authDisabled } from "@/auth";
import { DashboardShell } from "@/components/agent/dashboard-shell";

export default function DashboardLayout({ children }: { children: React.ReactNode }) {
  return (
    <DashboardShell authDisabled={authDisabled}>
      {children}
    </DashboardShell>
  );
}
