/**
 * ModelSelector — model selection dropdown for the ProviderSetup step.
 *
 * Displays a searchable list of models for the selected provider.
 * Used in the right panel to show available models after a provider
 * is configured and tested.
 */

import { useState } from 'react';
import { Search } from 'lucide-react';
import { cn } from '@/lib/utils';
import type { Model } from '../../types';

interface ModelSelectorProps {
  models: Model[];
  selectedModelId: string | null;
  onSelect: (modelId: string) => void;
  className?: string;
}

export function ModelSelector({
  models,
  selectedModelId,
  onSelect,
  className,
}: ModelSelectorProps) {
  const [searchQuery, setSearchQuery] = useState('');

  const filteredModels = searchQuery
    ? models.filter(
        (m) =>
          m.id.toLowerCase().includes(searchQuery.toLowerCase()) ||
          m.name.toLowerCase().includes(searchQuery.toLowerCase()),
      )
    : models;

  if (models.length === 0) {
    return (
      <div className="text-token-xs text-muted-foreground italic">
        无可用模型
      </div>
    );
  }

  return (
    <div className={cn('flex flex-col gap-2', className)}>
      {/* Search input */}
      <div className="relative">
        <Search className="absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
        <input
          type="text"
          placeholder="搜索模型..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full rounded-md border border-border bg-input pl-8 pr-3 py-1.5 text-token-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-brand-orange"
        />
      </div>

      {/* Model list */}
      <div className="flex flex-col gap-1 max-h-48 overflow-y-auto">
        {filteredModels.map((model) => {
          const isSelected = model.id === selectedModelId;
          return (
            <button
              key={model.id}
              type="button"
              onClick={() => onSelect(model.id)}
              className={cn(
                'flex items-center justify-between rounded-md px-2.5 py-1.5 text-left transition-colors',
                isSelected
                  ? 'bg-brand-orange/10 text-brand-orange'
                  : 'hover:bg-muted',
              )}
            >
              <div>
                <span className="text-token-sm font-medium text-foreground">
                  {model.name}
                </span>
                {model.context_window && (
                  <span className="text-token-xs text-muted-foreground ml-2">
                    {model.context_window >= 1000
                      ? `${(model.context_window / 1000).toFixed(0)}K`
                      : model.context_window}{' '}
                    ctx
                  </span>
                )}
              </div>
              {isSelected && (
                <span className="text-token-xs text-brand-orange font-medium">
                  已选
                </span>
              )}
            </button>
          );
        })}
        {filteredModels.length === 0 && searchQuery && (
          <p className="text-token-xs text-muted-foreground px-2 py-3 text-center">
            未找到匹配模型
          </p>
        )}
      </div>
    </div>
  );
}
