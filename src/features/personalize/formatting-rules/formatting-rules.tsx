import { useRef, useState } from 'react';
import { Page } from '@/components/page';
import { Typography } from '@/components/typography';
import { SettingsUI } from '@/components/settings-ui';
import { Switch } from '@/components/switch';
import { NumberInput } from '@/components/number-input';
import { Select, SelectTrigger, SelectValue, SelectContent, SelectItem } from '@/components/select';
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
import { Button } from '@/components/button';
import { getHospitalRuleExample, isHiddenHospitalRule } from './hospital-rule-presentation';
import type { RuleExample } from './hospital-rule-presentation';

interface SortableRuleCardProps {
    rule: FormattingRule;
    onUpdate: (id: string, updates: Partial<Omit<FormattingRule, 'id'>>) => void;
    onDelete: (id: string) => void;
    onDuplicate: (id: string) => void;
    example?: RuleExample;
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
    const {
        settings,
        isLoading,
        loadError,
        retryLoad,
        updateBuiltInOption,
        addRule,
        updateRule,
        deleteRule,
        duplicateRule,
        reorderRules,
    } = useFormattingRules();
    const thresholdInputRef = useRef<HTMLInputElement>(null);
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

    const visibleRules = settings.rules.filter((rule) => !isHiddenHospitalRule(rule));
    const activeRule = (activeId != null && visibleRules.find((rule) => rule.id === activeId)) || undefined;

    if (loadError) {
        return (
            <main className="space-y-8">
                <Page.Header>
                    <Typography.MainTitle data-testid="formatting-rules-title">
                        {t('Custom rules')}
                    </Typography.MainTitle>
                </Page.Header>
                <div role="alert" className="flex items-center gap-3">
                    <Typography.Paragraph>{t('Unable to load formatting rules.')}</Typography.Paragraph>
                    <Button variant="ghost" onClick={() => void retryLoad()} data-testid="formatting-rules-retry">
                        {t('Retry')}
                    </Button>
                </div>
            </main>
        );
    }

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

            <SettingsUI.Container>
                <SettingsUI.Item>
                    <SettingsUI.Description className="flex-1 min-w-0">
                        <Typography.Title>{t('Convert text numbers to digits')}</Typography.Title>
                        <Typography.Paragraph>
                            {t('Automatically converts numbers written in letters to digits.')}
                            <br />
                            <span className="text-xs italic text-muted-foreground">
                                {t('Example: "one" → "1", "twenty-three" → "23"')}
                            </span>
                        </Typography.Paragraph>
                    </SettingsUI.Description>
                    <Switch
                        checked={settings.built_in.convert_text_numbers}
                        onCheckedChange={(checked) => updateBuiltInOption('convert_text_numbers', checked)}
                        aria-label={t('Convert text numbers to digits')}
                        data-testid="option-convert-text-numbers"
                    />
                </SettingsUI.Item>
                {settings.built_in.convert_text_numbers && (
                    <>
                        <SettingsUI.Separator />
                        <SettingsUI.Item>
                            <SettingsUI.Description className="flex-1 min-w-0">
                                <Typography.Title>{t('Language for number conversion')}</Typography.Title>
                                <Typography.Paragraph>
                                    {t('Choose the language for text-to-number conversion')}
                                </Typography.Paragraph>
                            </SettingsUI.Description>
                            <Select
                                value={settings.built_in.text_numbers_language}
                                onValueChange={(value) => updateBuiltInOption('text_numbers_language', value)}
                            >
                                <SelectTrigger
                                    className="w-45"
                                    aria-label={t('Language for number conversion')}
                                    data-testid="number-conversion-language-select"
                                >
                                    <SelectValue placeholder={t('Select language')} />
                                </SelectTrigger>
                                <SelectContent>
                                    <SelectItem value="en">English</SelectItem>
                                    <SelectItem value="fr">Français</SelectItem>
                                    <SelectItem value="de">Deutsch</SelectItem>
                                    <SelectItem value="it">Italiano</SelectItem>
                                    <SelectItem value="es">Español</SelectItem>
                                    <SelectItem value="nl">Nederlands</SelectItem>
                                    <SelectItem value="pt">Português</SelectItem>
                                </SelectContent>
                            </Select>
                        </SettingsUI.Item>
                        <SettingsUI.Separator />
                        <SettingsUI.Item>
                            <SettingsUI.Description className="flex-1 min-w-0">
                                <Typography.Title>{t('Conversion threshold')}</Typography.Title>
                                <Typography.Paragraph>
                                    {t('Do not convert numbers that are strictly below this threshold.')}
                                </Typography.Paragraph>
                            </SettingsUI.Description>
                            <div className="w-30">
                                <NumberInput
                                    ref={thresholdInputRef}
                                    value={settings.built_in.text_numbers_threshold}
                                    onValueChange={(value) =>
                                        updateBuiltInOption('text_numbers_threshold', value ?? 10)
                                    }
                                    isAllowed={({ floatValue }) =>
                                        floatValue === undefined || (floatValue >= 0 && floatValue <= 50)
                                    }
                                    min={0}
                                    max={50}
                                    aria-label={t('Conversion threshold')}
                                    data-testid="number-conversion-threshold-input"
                                />
                            </div>
                        </SettingsUI.Item>
                    </>
                )}
            </SettingsUI.Container>

            <div id="custom-rules" className="space-y-4">
                <Typography.Paragraph>
                    {t('Replace words or patterns with your own text. Rules apply from top to bottom.')}
                    {visibleRules.length === 0 && (
                        <>
                            <br />
                            <span className="text-xs italic text-muted-foreground">
                                {t('Example: "semicolon" → ";"')}
                            </span>
                        </>
                    )}
                </Typography.Paragraph>
                {visibleRules.length > 0 && (
                    <DndContext
                        sensors={sensors}
                        collisionDetection={closestCenter}
                        onDragStart={handleDragStart}
                        onDragEnd={handleDragEnd}
                        modifiers={[restrictToVerticalAxis]}
                    >
                        <SortableContext
                            items={visibleRules.map((rule) => rule.id)}
                            strategy={verticalListSortingStrategy}
                        >
                            <div className="space-y-3">
                                {visibleRules.map((rule) => (
                                    <SortableRuleCard
                                        key={rule.id}
                                        rule={rule}
                                        onUpdate={updateRule}
                                        onDelete={deleteRule}
                                        onDuplicate={duplicateRule}
                                        example={getHospitalRuleExample(rule)}
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
                                    example={getHospitalRuleExample(activeRule)}
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
