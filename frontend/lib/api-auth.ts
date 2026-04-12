import { auth, authDisabled } from "@/auth";

const DEFAULT_USER_ID = "00000000-0000-0000-0000-000000000000";

export async function getRequiredUserId(): Promise<string | null> {
  if (authDisabled) return DEFAULT_USER_ID;
  const session = await auth();
  return session?.user?.id ?? null;
}
