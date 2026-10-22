UPDATE ai_provider_configs
SET endpoint = 'https://codeflow.sh/v1'
WHERE provider_id = 'codeflow'
    AND endpoint IN ('https://codeflow.asia/v1', 'https://cdn.codeflow.asia/v1');
