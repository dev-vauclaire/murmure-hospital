import assert from 'node:assert/strict';
import { test } from 'node:test';
import defaultFormattingRules from '../resources/hospital/default-formatting-rules.json';
import hospitalRuleExamples from '../src/features/personalize/formatting-rules/hospital-rule-examples.json';
import {
    HOSPITAL_RULE_PAIRS,
    HOSPITAL_RULE_UI_CONFIG,
    HOSPITAL_TECHNICAL_RULE_IDS,
    getHospitalRuleExample,
    isHiddenHospitalRule,
    isStandardHospitalRule,
} from '../src/features/personalize/formatting-rules/hospital-rule-presentation.ts';
import {
    deleteHospitalRule,
    reconcileHospitalRuleHelpers,
    reorderVisibleHospitalRules,
    updateHospitalRule,
} from '../src/features/personalize/formatting-rules/hospital-rule-settings.helpers.ts';

const cleanupIds = ['hospital-ponctuation-commandes-vocales', 'hospital-ponctuation-avant-commandes-vocales'];
const freshRules = () => globalThis.structuredClone(defaultFormattingRules.settings.rules);
const findRule = (rules, id) => {
    const rule = rules.find((candidate) => candidate.id === id);
    assert.ok(rule, `Missing synthetic/default test rule: ${id}`);
    return rule;
};
const personalRule = () => ({
    id: 'synthetic-personal-rule',
    name: 'Synthetic personal example',
    trigger: 'formule de fin',
    replacement: 'Bonne journée.',
    match_mode: 'smart',
    enabled: true,
});

test('the full model keeps 36 rules while the user sees 30 commands', () => {
    const rules = freshRules();
    const visible = rules.filter((rule) => !isHiddenHospitalRule(rule));
    assert.equal(rules.length, 36);
    assert.equal(HOSPITAL_TECHNICAL_RULE_IDS.size, 6);
    assert.equal(visible.length, 30);
    assert.deepEqual(new Set(rules.filter(isHiddenHospitalRule).map((rule) => rule.id)), HOSPITAL_TECHNICAL_RULE_IDS);

    const previous = HOSPITAL_RULE_UI_CONFIG.hideTechnicalRules;
    try {
        HOSPITAL_RULE_UI_CONFIG.hideTechnicalRules = false;
        assert.equal(rules.filter((rule) => !isHiddenHospitalRule(rule)).length, 36);
    } finally {
        HOSPITAL_RULE_UI_CONFIG.hideTechnicalRules = previous;
    }
    assert.equal(rules.length, 36);
});

test('every visible standard command has a concrete example and helpers do not', () => {
    const rules = freshRules();
    const visible = rules.filter((rule) => !isHiddenHospitalRule(rule));
    assert.deepEqual(new Set(Object.keys(hospitalRuleExamples)), new Set(visible.map((rule) => rule.id)));
    for (const rule of visible) {
        const example = getHospitalRuleExample(rule);
        assert.ok(example?.spoken.length > 0, rule.id);
        assert.ok(example.result.length > 0, rule.id);
        assert.equal(example.result, example.result.trimEnd(), rule.id);
    }
    for (const rule of rules.filter(isHiddenHospitalRule)) {
        assert.equal(getHospitalRuleExample(rule), undefined);
    }
    assert.ok(getHospitalRuleExample(findRule(rules, 'hospital-nouveau-paragraphe')).result.includes('\n\n'));
});

test('renaming or disabling a rule preserves its example, but changing its transformation removes it', () => {
    const rule = findRule(freshRules(), 'hospital-inserer-une-virgule');
    const expected = getHospitalRuleExample(rule);
    assert.deepEqual(getHospitalRuleExample({ ...rule, name: 'Mon libellé' }), expected);
    assert.deepEqual(getHospitalRuleExample({ ...rule, enabled: false }), expected);

    for (const change of [{ trigger: 'expression personnalisée' }, { replacement: ';' }, { match_mode: 'exact' }]) {
        const changed = { ...rule, ...change };
        assert.equal(isStandardHospitalRule(changed), false);
        assert.equal(getHospitalRuleExample(changed), undefined);
    }
});

test('a personal rule or duplicate ID never inherits hidden status or a bundled example', () => {
    const original = findRule(freshRules(), 'hospital-inserer-une-virgule');
    for (const id of ['synthetic-personal-duplicate', 'synthetic-personal-debut-ligne']) {
        const duplicate = { ...original, id };
        assert.equal(isHiddenHospitalRule(duplicate), false);
        assert.equal(isStandardHospitalRule(duplicate), false);
        assert.equal(getHospitalRuleExample(duplicate), undefined);
    }
    assert.equal(getHospitalRuleExample(personalRule()), undefined);
});

test('a coherent profile needs no reconciliation or persistence write', () => {
    const rules = freshRules();
    assert.strictEqual(reconcileHospitalRuleHelpers(rules), rules);
});

for (const [parentId, helperId] of Object.entries(HOSPITAL_RULE_PAIRS)) {
    test(`activation follows the visible command: ${parentId}`, () => {
        const rules = freshRules();
        const previous = globalThis.structuredClone(rules);
        const disabled = updateHospitalRule(rules, parentId, { enabled: false });
        assert.equal(disabled.length, 36);
        assert.equal(findRule(disabled, parentId).enabled, false);
        assert.equal(findRule(disabled, helperId).enabled, false);
        for (const cleanupId of cleanupIds) {
            assert.strictEqual(findRule(disabled, cleanupId), findRule(rules, cleanupId));
        }
        const enabled = updateHospitalRule(disabled, parentId, { enabled: true });
        assert.equal(findRule(enabled, parentId).enabled, true);
        assert.equal(findRule(enabled, helperId).enabled, true);
        assert.deepEqual(rules, previous);
    });
}

test('an inconsistent stored profile is reconciled without altering cleanup or personal rules', () => {
    const rules = [...freshRules(), personalRule()];
    for (const parentId of Object.keys(HOSPITAL_RULE_PAIRS)) findRule(rules, parentId).enabled = false;
    findRule(rules, cleanupIds[0]).enabled = false;
    const before = globalThis.structuredClone(rules);
    const reconciled = reconcileHospitalRuleHelpers(rules);
    assert.notStrictEqual(reconciled, rules);
    assert.equal(reconciled.length, rules.length);
    for (const helperId of Object.values(HOSPITAL_RULE_PAIRS)) {
        assert.equal(findRule(reconciled, helperId).enabled, false);
    }
    for (const id of [...cleanupIds, 'synthetic-personal-rule']) {
        assert.strictEqual(findRule(reconciled, id), findRule(rules, id));
    }
    assert.deepEqual(rules, before);
    assert.strictEqual(reconcileHospitalRuleHelpers(reconciled), reconciled);
});

test('personalizing a paired command disables its hidden helper until the standard transformation is restored', () => {
    const parentId = 'hospital-inserer-un-point-interrogation';
    const helperId = HOSPITAL_RULE_PAIRS[parentId];
    for (const change of [{ trigger: 'question personnalisée' }, { replacement: '?!' }, { match_mode: 'exact' }]) {
        const rules = freshRules();
        const standard = findRule(rules, parentId);
        const changed = updateHospitalRule(rules, parentId, change);
        assert.equal(findRule(changed, parentId).enabled, true);
        assert.equal(findRule(changed, helperId).enabled, false);
        assert.equal(getHospitalRuleExample(findRule(changed, parentId)), undefined);
        const restored = updateHospitalRule(changed, parentId, {
            trigger: standard.trigger,
            replacement: standard.replacement,
            match_mode: standard.match_mode,
        });
        assert.equal(findRule(restored, helperId).enabled, true);
        assert.ok(getHospitalRuleExample(findRule(restored, parentId)));
    }
});

test('renaming a paired command leaves its helper active', () => {
    const parentId = 'hospital-inserer-deux-points';
    const rules = updateHospitalRule(freshRules(), parentId, { name: 'Deux-points personnels' });
    assert.equal(findRule(rules, HOSPITAL_RULE_PAIRS[parentId]).enabled, true);
    assert.ok(getHospitalRuleExample(findRule(rules, parentId)));
});

test('an orphan or personalized hidden helper is disabled without discarding its contents', () => {
    const parentId = 'hospital-inserer-un-point-exclamation';
    const helperId = HOSPITAL_RULE_PAIRS[parentId];
    const orphan = freshRules().filter((rule) => rule.id !== parentId);
    assert.equal(findRule(reconcileHospitalRuleHelpers(orphan), helperId).enabled, false);

    const customized = freshRules();
    findRule(customized, helperId).trigger = '^expression personnelle';
    const fixed = findRule(reconcileHospitalRuleHelpers(customized), helperId);
    assert.equal(fixed.enabled, false);
    assert.equal(fixed.trigger, '^expression personnelle');
});

test('deleting a visible paired command removes its helper and keeps cleanup and personal rules', () => {
    for (const [parentId, helperId] of Object.entries(HOSPITAL_RULE_PAIRS)) {
        const rules = [...freshRules(), personalRule()];
        const before = globalThis.structuredClone(rules);
        const remaining = deleteHospitalRule(rules, parentId);
        assert.equal(remaining.length, rules.length - 2);
        assert.equal(
            remaining.some((rule) => rule.id === parentId || rule.id === helperId),
            false
        );
        for (const id of [...cleanupIds, 'synthetic-personal-rule']) {
            assert.strictEqual(findRule(remaining, id), findRule(rules, id));
        }
        assert.deepEqual(rules, before);
    }
});

test('deleting an ordinary personal rule does not remove technical rules', () => {
    const rules = [...freshRules(), personalRule()];
    const remaining = deleteHospitalRule(rules, 'synthetic-personal-rule');
    assert.equal(remaining.length, 36);
    assert.equal(remaining.filter(isHiddenHospitalRule).length, 6);
    assert.strictEqual(deleteHospitalRule(rules, 'unknown-id'), rules);
    assert.strictEqual(updateHospitalRule(rules, 'unknown-id', { enabled: false }), rules);
});

test('reordering visible commands keeps every technical execution slot fixed, including scattered helpers', () => {
    const model = freshRules();
    const hidden = model.filter(isHiddenHospitalRule);
    const visible = model.filter((rule) => !isHiddenHospitalRule(rule));
    const rules = [];
    for (let index = 0; index < visible.length; index += 1) {
        rules.push(visible[index]);
        if (index < hidden.length) rules.push(hidden[index]);
    }
    for (const rule of rules) Object.freeze(rule);
    Object.freeze(rules);

    const first = visible[0].id;
    const last = visible.at(-1).id;
    const reordered = reorderVisibleHospitalRules(rules, first, last);
    assert.equal(reordered.length, 36);
    assert.deepEqual(
        reordered.filter((rule) => !isHiddenHospitalRule(rule)).map((rule) => rule.id),
        [...visible.slice(1), visible[0]].map((rule) => rule.id)
    );
    rules.forEach((rule, index) => {
        if (isHiddenHospitalRule(rule)) assert.strictEqual(reordered[index], rule);
    });
    assert.deepEqual(new Set(reordered), new Set(rules));
});

test('a hidden or unknown drag target is rejected without changing the saved list', () => {
    const rules = freshRules();
    const visibleId = rules.find((rule) => !isHiddenHospitalRule(rule)).id;
    const hiddenId = [...HOSPITAL_TECHNICAL_RULE_IDS][0];
    for (const [activeId, overId] of [
        [hiddenId, visibleId],
        [visibleId, hiddenId],
        ['unknown-id', visibleId],
        [visibleId, 'unknown-id'],
        [visibleId, visibleId],
    ]) {
        assert.strictEqual(reorderVisibleHospitalRules(rules, activeId, overId), rules);
    }
});
