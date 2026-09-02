import { defineRailway, image, project, service, volume } from "railway/iac";

const optionalEnvironment = [
  "KEITH_WEB_LOGIN_SECRET",
  "KEITH_PUBLIC_ORIGIN",
  "KEITH_CREDENTIAL_KEY",
  "KEITH_OPENAI_COMPAT_API_KEY",
  "KEITH_PLATFORM_API_KEY",
  "OPENAI_API_KEY",
  "ANTHROPIC_API_KEY",
  "OPENROUTER_API_KEY",
  "GEMINI_API_KEY",
  "AZURE_OPENAI_API_KEY",
  "AWS_BEARER_TOKEN_BEDROCK",
  "PRIME_API_KEY",
  "GOOGLE_CLOUD_API_KEY",
  "DEEPSEEK_API_KEY",
  "MISTRAL_API_KEY",
  "GROQ_API_KEY",
  "CEREBRAS_API_KEY",
  "XAI_API_KEY",
  "AI_GATEWAY_API_KEY",
  "ZAI_API_KEY",
  "OPENCODE_API_KEY",
  "HF_TOKEN",
  "FIREWORKS_API_KEY",
  "KIMI_API_KEY",
  "MINIMAX_API_KEY",
  "MINIMAX_CN_API_KEY",
  "MOONSHOT_API_KEY",
  "XIAOMI_API_KEY",
  "XIAOMI_TOKEN_PLAN_CN_API_KEY",
  "XIAOMI_TOKEN_PLAN_AMS_API_KEY",
  "XIAOMI_TOKEN_PLAN_SGP_API_KEY",
  "CLOUDFLARE_API_KEY",
  "DASHSCOPE_API_KEY",
  "QIANFAN_API_KEY",
  "ZHIPU_API_KEY",
  "MODELSCOPE_API_KEY",
  "ARK_API_KEY",
  "MIMO_API_KEY",
  "LINKAI_API_KEY",
  "CUSTOM_API_KEY",
] as const;

export default defineRailway(() => {
  const suppliedEnvironment = Object.fromEntries(
    optionalEnvironment.flatMap((name) => {
      const value = process.env[name];
      return value ? [[name, value]] : [];
    }),
  );

  const keith = service("keith", {
    source: image(process.env.KEITH_IMAGE ?? "ghcr.io/machinecity/keith-agent:main"),
    env: {
      PORT: "7341",
      KEITH_DATA_ROOT: "/var/lib/keith",
      ...suppliedEnvironment,
    },
    volumeMounts: {
      "/var/lib/keith": volume("keith-data", { sizeMB: 20480 }),
    },
  });

  return project("keith", { resources: [keith] });
});
