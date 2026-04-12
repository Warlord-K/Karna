import { NextResponse } from "next/server";
import { auth } from "@/auth";
import { readFile } from "fs/promises";
import { parse } from "yaml";

let cachedConfig: any | null = null;

async function loadConfig() {
  if (cachedConfig) return cachedConfig;

  const configPaths = [
    process.env.CONFIG_PATH,
    "/etc/karna/config.yaml",
    "./config.yaml",
    "../config.yaml",
  ].filter(Boolean) as string[];

  for (const path of configPaths) {
    try {
      const content = await readFile(path, "utf-8");
      cachedConfig = parse(content);
      return cachedConfig;
    } catch {
      continue;
    }
  }

  return { repos: [] };
}

export async function GET() {
  const session = await auth();
  if (!session?.user?.id) {
    return NextResponse.json({ error: "Unauthorized" }, { status: 401 });
  }

  const config = await loadConfig();

  const repos = (config?.repos || []).map((r: any) => ({
    repo: r.repo,
    name: r.repo?.split("/").pop() || r.repo,
    branch: r.branch || "main",
  }));

  // Parse backends from agent config
  const rawBackends = config?.agent?.backends || {};
  const backends: Record<string, { models: string[]; default_model: string }> = {};
  for (const [name, cfg] of Object.entries(rawBackends) as [string, any][]) {
    backends[name] = {
      models: cfg?.models || [],
      default_model: cfg?.default_model || cfg?.models?.[0] || "",
    };
  }

  // Fallback if no backends configured
  if (Object.keys(backends).length === 0) {
    backends.claude = { models: ["haiku", "sonnet", "opus"], default_model: "sonnet" };
  }

  // Parse skill names
  const skills: string[] = [];
  // Inline skills from config
  if (Array.isArray(config?.skills)) {
    for (const s of config.skills) {
      if (s?.name) skills.push(s.name);
    }
  }

  // Parse MCP server names
  const mcpServers: string[] = [];
  if (Array.isArray(config?.mcp_servers)) {
    for (const s of config.mcp_servers) {
      if (s?.name) mcpServers.push(s.name);
    }
  }

  return NextResponse.json({ repos, backends, skills, mcpServers });
}
