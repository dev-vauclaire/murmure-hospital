import { HOSPITAL_RULE_PAIRS, isHiddenHospitalRule, isStandardHospitalRule } from './hospital-rule-presentation';
import type { FormattingRule } from './types';

const pairedHelpers = new Map(Object.entries(HOSPITAL_RULE_PAIRS));
const parentRules = new Map(Object.entries(HOSPITAL_RULE_PAIRS).map(([parent, helper]) => [helper, parent]));

/** Hidden helpers follow the command they implement, never an obsolete custom trigger. */
export function reconcileHospitalRuleHelpers(rules: FormattingRule[]): FormattingRule[] {
    const rulesById = new Map(rules.map((rule) => [rule.id, rule]));
    let changed = false;
    const reconciled = rules.map((rule) => {
        const parentId = parentRules.get(rule.id);
        if (parentId === undefined) return rule;

        const parent = rulesById.get(parentId);
        const enabled =
            parent !== undefined && parent.enabled && isStandardHospitalRule(parent) && isStandardHospitalRule(rule);
        if (rule.enabled === enabled) return rule;

        changed = true;
        return { ...rule, enabled };
    });

    return changed ? reconciled : rules;
}

export function updateHospitalRule(
    rules: FormattingRule[],
    id: string,
    updates: Partial<Omit<FormattingRule, 'id'>>
): FormattingRule[] {
    if (!rules.some((rule) => rule.id === id)) return rules;

    const updated = rules.map((rule) => (rule.id === id ? { ...rule, ...updates } : rule));
    return reconcileHospitalRuleHelpers(updated);
}

export function deleteHospitalRule(rules: FormattingRule[], id: string): FormattingRule[] {
    if (!rules.some((rule) => rule.id === id)) return rules;

    const helperId = pairedHelpers.get(id);
    return reconcileHospitalRuleHelpers(rules.filter((rule) => rule.id !== id && rule.id !== helperId));
}

/** Reorder the visible slots only; technical rules keep their execution positions. */
export function reorderVisibleHospitalRules(
    rules: FormattingRule[],
    activeId: string,
    overId: string
): FormattingRule[] {
    const visible = rules.filter((rule) => !isHiddenHospitalRule(rule));
    const oldIndex = visible.findIndex((rule) => rule.id === activeId);
    const newIndex = visible.findIndex((rule) => rule.id === overId);
    if (oldIndex === -1 || newIndex === -1 || oldIndex === newIndex) return rules;

    const [moved] = visible.splice(oldIndex, 1);
    visible.splice(newIndex, 0, moved);
    let visibleIndex = 0;
    const reordered = rules.map((rule) => (isHiddenHospitalRule(rule) ? rule : visible[visibleIndex++]));
    return reconcileHospitalRuleHelpers(reordered);
}
