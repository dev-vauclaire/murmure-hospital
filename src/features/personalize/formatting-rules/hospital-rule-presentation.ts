import defaultFormattingRules from '../../../../resources/hospital/default-formatting-rules.json';
import hospitalRuleExamples from './hospital-rule-examples.json';
import type { FormattingRule } from './types';

export interface RuleExample {
    spoken: string;
    result: string;
}

export const HOSPITAL_RULE_UI_CONFIG = {
    hideTechnicalRules: true,
};

export const HOSPITAL_RULE_PAIRS: Readonly<Record<string, string>> = {
    'hospital-inserer-un-point-virgule': 'hospital-inserer-un-point-virgule-debut-ligne',
    'hospital-inserer-deux-points': 'hospital-inserer-deux-points-debut-ligne',
    'hospital-inserer-un-point-interrogation': 'hospital-inserer-un-point-interrogation-debut-ligne',
    'hospital-inserer-un-point-exclamation': 'hospital-inserer-un-point-exclamation-debut-ligne',
};

export const HOSPITAL_TECHNICAL_RULE_IDS: ReadonlySet<string> = new Set([
    'hospital-ponctuation-commandes-vocales',
    'hospital-ponctuation-avant-commandes-vocales',
    ...Object.values(HOSPITAL_RULE_PAIRS),
]);

const standardRules = new Map(defaultFormattingRules.settings.rules.map((rule) => [rule.id, rule]));
const examples: Readonly<Partial<Record<string, RuleExample>>> = hospitalRuleExamples;

export function isHiddenHospitalRule(rule: FormattingRule): boolean {
    return HOSPITAL_RULE_UI_CONFIG.hideTechnicalRules && HOSPITAL_TECHNICAL_RULE_IDS.has(rule.id);
}

export function isStandardHospitalRule(rule: FormattingRule): boolean {
    const standard = standardRules.get(rule.id);
    // Display names and activation are user preferences; they do not change
    // the transformation described by the bundled rule.
    return (
        standard !== undefined &&
        rule.trigger === standard.trigger &&
        rule.replacement === standard.replacement &&
        rule.match_mode === standard.match_mode
    );
}

export function getHospitalRuleExample(rule: FormattingRule): RuleExample | undefined {
    return isStandardHospitalRule(rule) ? examples[rule.id] : undefined;
}
