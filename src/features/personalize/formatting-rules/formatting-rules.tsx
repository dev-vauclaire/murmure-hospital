import { useState } from 'react';
import { Page } from '@/components/page';
import { Typography } from '@/components/typography';
import { useTranslation } from '@/i18n';
import { useFormattingRules } from './hooks/use-formatting-rules';
import { RuleCard } from '../../../components/rule-card';
import { AddRuleSection } from '../../../components/add-rule-section';
import {
    DndContext,
    closestCenter,
    DragEndEvent,
    DragStartEvent,
    DragOverlay,
    PointerSensor,
    KeyboardSensor,
    useSensor,
    useSensors,
} from '@dnd-kit/core';
import {
    SortableContext,
    verticalListSortingStrategy,
    useSortable,
    sortableKeyboardCoordinates,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import { FormattingRule } from './types';

interface SortableRuleCardProps {
    rule: FormattingRule;
    onUpdate: (id: string, updates: Partial<Omit<FormattingRule, 'id'>>) => void;
    onDelete: (id: string) => void;
    onDuplicate: (id: string) => void;
}

const SortableRuleCard = ({ rule, ...props }: SortableRuleCardProps) => {
    const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: rule.id });

    const style = {
        transform: CSS.Transform.toString(transform),
        transition,
    };

    return (
        <div ref={setNodeRef} style={style}>
            {isDragging ? (
                <div className="border border-dashed border-border rounded-lg h-14 bg-card/10" />
            ) : (
                <RuleCard rule={rule} {...props} dragHandleProps={{ ...attributes, ...listeners }} />
            )}
        </div>
    );
};

export const FormattingRules = () => {
    const { t } = useTranslation();
    const { settings, isLoading, addRule, updateRule, deleteRule, duplicateRule, reorderRules } = useFormattingRules();
    const [activeId, setActiveId] = useState<string | null>(null);

    const sensors = useSensors(
        useSensor(PointerSensor, {
            activationConstraint: { distance: 8 },
        }),
        useSensor(KeyboardSensor, {
            coordinateGetter: sortableKeyboardCoordinates,
        })
    );

    const handleDragStart = (event: DragStartEvent) => {
        setActiveId(String(event.active.id));
    };

    const handleDragEnd = (event: DragEndEvent) => {
        setActiveId(null);
        const { active, over } = event;
        if (over != null && active.id !== over.id) {
            reorderRules(String(active.id), String(over.id));
        }
    };

    const activeRule = (activeId != null && settings.rules.find((rule) => rule.id === activeId)) || undefined;

    if (isLoading) {
        return (
            <main className="space-y-8">
                <Page.Header>
                    <Typography.MainTitle data-testid="formatting-rules-title">
                        {t('Custom rules')}
                    </Typography.MainTitle>
                </Page.Header>
                <div className="text-muted-foreground">{t('Loading...')}</div>
            </main>
        );
    }

    return (
        <main className="space-y-6">
            <Page.Header>
                <Typography.MainTitle data-testid="formatting-rules-title">{t('Custom rules')}</Typography.MainTitle>
            </Page.Header>

            <div id="custom-rules" className="space-y-4">
                <Typography.Paragraph>
                    {t('Replace words or patterns with your own text. Rules apply from top to bottom.')}
                    {settings.rules.length === 0 && (
                        <>
                            <br />
                            <span className="text-xs italic text-muted-foreground">
                                {t('Example: "semicolon" → ";"')}
                            </span>
                        </>
                    )}
                </Typography.Paragraph>
                {settings.rules.length > 0 && (
                    <DndContext
                        sensors={sensors}
                        collisionDetection={closestCenter}
                        onDragStart={handleDragStart}
                        onDragEnd={handleDragEnd}
                        modifiers={[restrictToVerticalAxis]}
                    >
                        <SortableContext
                            items={settings.rules.map((rule) => rule.id)}
                            strategy={verticalListSortingStrategy}
                        >
                            <div className="space-y-3">
                                {settings.rules.map((rule) => (
                                    <SortableRuleCard
                                        key={rule.id}
                                        rule={rule}
                                        onUpdate={updateRule}
                                        onDelete={deleteRule}
                                        onDuplicate={duplicateRule}
                                    />
                                ))}
                            </div>
                        </SortableContext>
                        <DragOverlay>
                            {activeRule != null && (
                                <RuleCard
                                    rule={activeRule}
                                    onUpdate={updateRule}
                                    onDelete={deleteRule}
                                    onDuplicate={duplicateRule}
                                />
                            )}
                        </DragOverlay>
                    </DndContext>
                )}
                <AddRuleSection onAdd={addRule} />
            </div>
        </main>
    );
};
