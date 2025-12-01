import { CompletionParams } from '@/components/ModelSettingsModal';

/**
 * Default completion parameters
 */
export const DEFAULT_COMPLETION_PARAMS: CompletionParams = {
  temperature: null,
  top_p: null,
  max_tokens: 2048,
  repeat_penalty: 1.1,
  repeat_last_n: 64,
  chat_template_kwargs: {
    reasoning_effort: "low"
  },
};

/**
 * Migrate completion params from localStorage to database (one-time migration)
 */
async function migrateFromLocalStorage(): Promise<void> {
  if (typeof window === 'undefined') {
    return;
  }

  try {
    const saved = localStorage.getItem('completionParams');
    if (!saved) {
      return;
    }

    const { invoke } = await import('@tauri-apps/api/core');
    // Check if DB already has completion params
    const dbParams = await invoke<string | null>('api_get_completion_params', {});

    // Only migrate if DB doesn't have it yet
    if (!dbParams) {
      await invoke('api_save_completion_params', {
        completionParams: saved,
      });
      // Remove from localStorage after successful migration
      localStorage.removeItem('completionParams');
    }
  } catch (e) {
    // Silently fail - migration is best effort
    console.error('[migrateFromLocalStorage] Failed to migrate:', e);
  }
}

/**
 * Load completion parameters from database
 * Performs one-time migration from localStorage if needed
 */
export async function loadCompletionParams(): Promise<CompletionParams> {
  if (typeof window === 'undefined') {
    return DEFAULT_COMPLETION_PARAMS;
  }

  // One-time migration from localStorage to DB
  await migrateFromLocalStorage();

  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const dbParams = await invoke<string | null>('api_get_completion_params', {});

    if (dbParams) {
      const parsed = JSON.parse(dbParams) as CompletionParams;
      return mergeWithDefaults(parsed);
    }
  } catch (e) {
    console.error('[loadCompletionParams] Failed to load from database:', e);
  }

  return DEFAULT_COMPLETION_PARAMS;
}

/**
 * Merge parsed params with defaults
 */
function mergeWithDefaults(parsed: CompletionParams): CompletionParams {
  return {
    temperature: parsed.temperature ?? DEFAULT_COMPLETION_PARAMS.temperature,
    top_p: parsed.top_p ?? DEFAULT_COMPLETION_PARAMS.top_p,
    max_tokens: parsed.max_tokens ?? DEFAULT_COMPLETION_PARAMS.max_tokens,
    repeat_penalty: parsed.repeat_penalty ?? DEFAULT_COMPLETION_PARAMS.repeat_penalty,
    repeat_last_n: parsed.repeat_last_n ?? DEFAULT_COMPLETION_PARAMS.repeat_last_n,
    chat_template_kwargs: parsed.chat_template_kwargs || DEFAULT_COMPLETION_PARAMS.chat_template_kwargs,
  };
}

