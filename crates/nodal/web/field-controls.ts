import {
  BUS_FIELDS,
  EQUIPMENT_FIELDS,
  fieldOptions,
  type FieldDefinition,
  type FieldTarget,
  type FilterOperator,
  type FilterRule,
} from './field-view';
import {
  COLOR_PALETTES,
  defaultColorSettings,
  loadColorSettings,
  saveColorSettings,
  type ColorScheme,
  type ColorSettings,
} from './field-colors';
import type { Snapshot } from './types';

type ChangeKind = 'filters' | 'colors';
type FormState = Map<string, string>;
type FocusState = { key: string; start: number | null; end: number | null } | null;

const OPERATORS: { value: FilterOperator; label: string }[] = [
  { value: 'eq', label: 'is' },
  { value: 'neq', label: 'is not' },
  { value: 'contains', label: 'contains' },
  { value: 'gt', label: 'greater than' },
  { value: 'gte', label: 'at least' },
  { value: 'lt', label: 'less than' },
  { value: 'lte', label: 'at most' },
  { value: 'between', label: 'between' },
  { value: 'missing', label: 'is missing' },
  { value: 'present', label: 'is present' },
];

let nextFilterId = 1;

function make<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  return node;
}

function labelFor(text: string, control: HTMLElement): HTMLLabelElement {
  const label = make('label');
  label.append(document.createTextNode(text), control);
  return label;
}

function option(select: HTMLSelectElement, value: string, label: string): void {
  const item = make('option');
  item.value = value;
  item.textContent = label;
  select.append(item);
}

function stateKey(control: HTMLElement): string | null {
  return control.dataset.stateKey ?? null;
}

function readState(root: HTMLElement): FormState {
  const values: FormState = new Map();
  for (const control of root.querySelectorAll<HTMLInputElement | HTMLSelectElement>('[data-state-key]')) {
    const key = stateKey(control);
    if (key) values.set(key, control.value);
  }
  return values;
}

function saveState(control: HTMLInputElement | HTMLSelectElement, values: FormState): void {
  const key = stateKey(control);
  if (key) values.set(key, control.value);
}

function restoreValue<T extends HTMLInputElement | HTMLSelectElement>(control: T, key: string, values: FormState): T {
  control.dataset.stateKey = key;
  const value = values.get(key);
  if (value !== undefined) control.value = value;
  return control;
}

function focusState(root: HTMLElement): FocusState {
  const active = document.activeElement;
  if (!(active instanceof HTMLInputElement || active instanceof HTMLSelectElement) || !root.contains(active)) return null;
  return {
    key: active.dataset.stateKey ?? '',
    start: active instanceof HTMLInputElement ? active.selectionStart : null,
    end: active instanceof HTMLInputElement ? active.selectionEnd : null,
  };
}

function restoreFocus(root: HTMLElement, focus: FocusState): void {
  if (!focus?.key) return;
  const control = [...root.querySelectorAll<HTMLInputElement | HTMLSelectElement>('[data-state-key]')]
    .find(candidate => candidate.dataset.stateKey === focus.key);
  if (!control) return;
  control.focus();
  if (control instanceof HTMLInputElement && focus.start !== null && focus.end !== null) {
    try { control.setSelectionRange(focus.start, focus.end); } catch { /* color and number inputs do not expose a text selection */ }
  }
}

function definitions(target: FieldTarget): FieldDefinition[] {
  return target === 'bus' ? BUS_FIELDS : EQUIPMENT_FIELDS;
}

function schemeFields(target: FieldTarget): string {
  return `${target}-colors`;
}

function validColor(value: string): boolean {
  return /^#[0-9a-f]{6}$/i.test(value);
}

function paletteColors(paletteId: string): string[] {
  return COLOR_PALETTES.find(palette => palette.id === paletteId)?.colors ?? COLOR_PALETTES[0]?.colors ?? ['#4e79a7', '#f28e2b'];
}

function isNumericField(target: FieldTarget, field: string): boolean {
  return definitions(target).find(definition => definition.key === field)?.type === 'number';
}

function describeRule(rule: FilterRule): string {
  const definition = definitions(rule.target).find(field => field.key === rule.field);
  const operator = OPERATORS.find(item => item.value === rule.operator)?.label ?? rule.operator;
  if (rule.operator === 'between') return `${rule.target === 'bus' ? 'Bus' : 'Equipment'} ${definition?.label ?? rule.field} ${operator} ${rule.value} and ${rule.upper ?? ''}`;
  if (rule.operator === 'missing' || rule.operator === 'present') return `${rule.target === 'bus' ? 'Bus' : 'Equipment'} ${definition?.label ?? rule.field} ${operator}`;
  return `${rule.target === 'bus' ? 'Bus' : 'Equipment'} ${definition?.label ?? rule.field} ${operator} ${rule.value}`;
}

function controlInput(type: string, key: string, values: FormState, className?: string): HTMLInputElement {
  const input = make('input', className);
  input.type = type;
  return restoreValue(input, key, values);
}

function controlSelect(key: string, values: FormState, className?: string): HTMLSelectElement {
  return restoreValue(make('select', className), key, values);
}

function section(title: string): HTMLElement {
  const sectionNode = make('section', 'field-controls__section');
  const heading = make('h3');
  heading.textContent = title;
  sectionNode.append(heading);
  return sectionNode;
}

function createColorScheme(settings: ColorSettings, target: FieldTarget): ColorScheme {
  return settings[target];
}

/** Accessible filter and field-color controls for the current case snapshot. */
export class FieldControls {
  public filters: FilterRule[] = [];
  public colors: ColorSettings = loadColorSettings();

  private container: HTMLElement | null = null;
  private snapshot: Snapshot | null = null;
  private onChange: ((kind: ChangeKind) => void) | null = null;
  private formState: FormState = new Map();
  private expanded: boolean | null = null;
  private pendingValidation = '';
  private categorySelection: Record<FieldTarget, string> = { bus: '', equipment: '' };

  mount(container: HTMLElement, snapshot: Snapshot, onChange: (kind: ChangeKind) => void): void {
    if (this.container) {
      this.formState = readState(this.container);
      const details = this.container.querySelector<HTMLDetailsElement>('details.field-controls');
      if (details) this.expanded = details.open;
      for (const target of ['bus', 'equipment'] as const) {
        const selection = this.container.querySelector<HTMLSelectElement>(`[data-category-target="${target}"]`);
        if (selection) this.categorySelection[target] = selection.value;
      }
    }
    this.container = container;
    this.snapshot = snapshot;
    this.onChange = onChange;
    this.render();
  }

  resetFilters(): void {
    const details = this.container?.querySelector<HTMLDetailsElement>('details.field-controls');
    if (details) this.expanded = details.open;
    this.filters = [];
    this.pendingValidation = '';
    this.formState = new Map();
    this.categorySelection = { bus: '', equipment: '' };
    this.container = null;
    this.snapshot = null;
    this.onChange = null;
  }

  private emit(kind: ChangeKind): void {
    this.onChange?.(kind);
  }

  private refresh(kind: ChangeKind): void {
    this.render();
    this.emit(kind);
  }

  private render(): void {
    const container = this.container;
    const snapshot = this.snapshot;
    if (!container || !snapshot) return;
    const previous = container.querySelector<HTMLElement>('.field-controls');
    const focus = previous ? focusState(previous) : null;
    const details = make('details', 'field-controls');
    const previousOpen = previous instanceof HTMLDetailsElement ? previous.open : previous?.querySelector<HTMLDetailsElement>('details')?.open;
    details.open = previousOpen ?? this.expanded ?? false;
    details.addEventListener('toggle', () => {
      if (container.querySelector('.field-controls') === details) this.expanded = details.open;
    });
    const summary = make('summary');
    summary.textContent = 'Filters & colors';
    details.append(summary);

    const body = make('div', 'field-controls__body');
    body.append(this.renderFilters(snapshot), this.renderColorSection(snapshot, 'bus'), this.renderColorSection(snapshot, 'equipment'));
    details.append(body);
    container.replaceChildren(details);
    restoreFocus(details, focus);
  }

  private renderFilters(snapshot: Snapshot): HTMLElement {
    const sectionNode = section('Field filters');
    const targetSelect = controlSelect('filter-target', this.formState);
    targetSelect.setAttribute('aria-label', 'Filter target');
    option(targetSelect, 'bus', 'Buses');
    option(targetSelect, 'equipment', 'Equipment');
    const savedTarget = this.formState.get('filter-target');
    targetSelect.value = savedTarget === 'equipment' ? 'equipment' : 'bus';
    this.formState.set('filter-target', targetSelect.value);
    const fieldSelect = controlSelect('filter-field', this.formState);
    fieldSelect.setAttribute('aria-label', 'Filter field');
    const operatorSelect = controlSelect('filter-operator', this.formState);
    operatorSelect.setAttribute('aria-label', 'Filter operator');
    const valueHost = make('div', 'field-controls__filter-value');
    const upperHost = make('div', 'field-controls__filter-upper');
    const form = make('form', 'field-controls__filter-form');
    form.noValidate = true;
    const validation = make('p', 'field-controls__validation');
    validation.setAttribute('role', 'status');
    validation.setAttribute('aria-live', 'polite');
    validation.textContent = this.pendingValidation;
    const add = make('button');
    add.type = 'submit';
    add.textContent = 'Add filter';
    form.append(
      labelFor('Filter target', targetSelect),
      labelFor('Filter field', fieldSelect),
      labelFor('Filter operator', operatorSelect),
      valueHost,
      upperHost,
      validation,
      add,
    );

    const refreshFilterOptions = (): void => {
      const target = targetSelect.value as FieldTarget;
      const fields = definitions(target);
      const desiredField = this.formState.get('filter-field');
      const priorField = fieldSelect.value;
      fieldSelect.replaceChildren();
      for (const field of fields) option(fieldSelect, field.key, field.label);
      const selectedField = fields.find(field => field.key === desiredField)?.key
        ?? fields.find(field => field.key === priorField)?.key
        ?? fields[0]?.key
        ?? '';
      fieldSelect.value = selectedField;
      this.formState.set('filter-field', fieldSelect.value);
      const selected = fields.find(field => field.key === fieldSelect.value) ?? fields[0];
      const desiredOperator = this.formState.get('filter-operator');
      const priorOperator = operatorSelect.value;
      const allowed = selected?.type === 'number'
        ? OPERATORS.filter(item => ['eq', 'neq', 'gt', 'gte', 'lt', 'lte', 'between', 'missing', 'present'].includes(item.value))
        : selected?.type === 'text'
          ? OPERATORS.filter(item => ['eq', 'neq', 'contains', 'missing', 'present'].includes(item.value))
          : OPERATORS.filter(item => ['eq', 'neq', 'missing', 'present'].includes(item.value));
      operatorSelect.replaceChildren();
      for (const item of allowed) option(operatorSelect, item.value, item.label);
      const selectedOperator = allowed.find(item => item.value === desiredOperator)?.value
        ?? allowed.find(item => item.value === priorOperator)?.value
        ?? allowed[0]?.value
        ?? 'eq';
      operatorSelect.value = selectedOperator;
      this.formState.set('filter-operator', operatorSelect.value);
      this.renderFilterValues(snapshot, target, selected, operatorSelect.value as FilterOperator, valueHost, upperHost);
    };

    targetSelect.addEventListener('change', () => {
      saveState(targetSelect, this.formState);
      this.formState.delete('filter-field');
      this.formState.delete('filter-operator');
      refreshFilterOptions();
    });
    fieldSelect.addEventListener('change', () => {
      saveState(fieldSelect, this.formState);
      refreshFilterOptions();
    });
    operatorSelect.addEventListener('change', () => {
      saveState(operatorSelect, this.formState);
      const target = targetSelect.value as FieldTarget;
      const selected = definitions(target).find(field => field.key === fieldSelect.value);
      this.renderFilterValues(snapshot, target, selected, operatorSelect.value as FilterOperator, valueHost, upperHost);
    });
    refreshFilterOptions();

    form.addEventListener('input', event => {
      const control = event.target;
      if (control instanceof HTMLInputElement || control instanceof HTMLSelectElement) saveState(control, this.formState);
    });
    form.addEventListener('change', event => {
      const control = event.target;
      if (control instanceof HTMLInputElement || control instanceof HTMLSelectElement) saveState(control, this.formState);
    });
    form.addEventListener('submit', event => {
      event.preventDefault();
      const target = targetSelect.value as FieldTarget;
      const field = definitions(target).find(item => item.key === fieldSelect.value);
      const operator = operatorSelect.value as FilterOperator;
      const valueControl = valueHost.querySelector<HTMLInputElement | HTMLSelectElement>('input, select');
      const upperControl = upperHost.querySelector<HTMLInputElement>('input');
      const value = valueControl?.value ?? '';
      const upper = upperControl?.value ?? '';
      if (!field) return;
      if (operator !== 'missing' && operator !== 'present') {
        if (!value.trim()) {
          this.pendingValidation = 'Enter a filter value.';
          this.render();
          return;
        }
        if (field.type === 'number' && !Number.isFinite(Number(value))) {
          this.pendingValidation = 'Enter a finite number.';
          this.render();
          return;
        }
      }
      if (operator === 'between') {
        if (!upper.trim() || !Number.isFinite(Number(upper))) {
          this.pendingValidation = 'Enter a finite upper bound.';
          this.render();
          return;
        }
        if (Number(value) > Number(upper)) {
          this.pendingValidation = 'The lower bound must be less than or equal to the upper bound.';
          this.render();
          return;
        }
      }
      this.filters.push({
        id: `field-filter-${Date.now()}-${nextFilterId++}`,
        target,
        field: field.key,
        operator,
        value,
        ...(operator === 'between' ? { upper } : {}),
      });
      this.pendingValidation = '';
      this.refresh('filters');
    });

    sectionNode.append(form);
    const list = make('div', 'field-controls__active-filters');
    const count = make('p', 'field-controls__filter-count');
    count.textContent = `${this.filters.length} ${this.filters.length === 1 ? 'filter' : 'filters'}`;
    list.append(count);
    for (const rule of this.filters) {
      const chip = make('div', 'field-controls__filter-chip');
      const description = make('span');
      description.textContent = describeRule(rule);
      const remove = make('button');
      remove.type = 'button';
      remove.textContent = 'Remove';
      remove.setAttribute('aria-label', `Remove filter: ${describeRule(rule)}`);
      remove.addEventListener('click', () => {
        this.filters = this.filters.filter(candidate => candidate.id !== rule.id);
        this.refresh('filters');
      });
      chip.append(description, remove);
      list.append(chip);
    }
    const clear = make('button');
    clear.type = 'button';
    clear.textContent = 'Clear filters';
    clear.disabled = this.filters.length === 0;
    clear.addEventListener('click', () => {
      this.filters = [];
      this.pendingValidation = '';
      this.refresh('filters');
    });
    list.append(clear);
    sectionNode.append(list);
    return sectionNode;
  }

  private renderFilterValues(
    snapshot: Snapshot,
    target: FieldTarget,
    field: FieldDefinition | undefined,
    operator: FilterOperator,
    valueHost: HTMLElement,
    upperHost: HTMLElement,
  ): void {
    valueHost.replaceChildren();
    upperHost.replaceChildren();
    if (!field || operator === 'missing' || operator === 'present') return;
    const key = 'filter-value';
    if (field.type === 'category') {
      const select = controlSelect(key, this.formState);
      select.setAttribute('aria-label', 'Filter value');
      for (const value of fieldOptions(snapshot, target, field.key)) {
        if (value !== null) option(select, String(value), String(value));
      }
      if (!select.options.length) option(select, '', '(no values)');
      const wanted = this.formState.get(key);
      select.value = wanted !== undefined && [...select.options].some(item => item.value === wanted)
        ? wanted
        : select.options[0]?.value ?? '';
      this.formState.set(key, select.value);
      valueHost.append(labelFor('Filter value', select));
    } else {
      const input = controlInput(field.type === 'number' ? 'number' : 'text', key, this.formState);
      input.setAttribute('aria-label', 'Filter value');
      if (field.type === 'number') input.step = 'any';
      valueHost.append(labelFor('Filter value', input));
    }
    if (operator === 'between') {
      const upper = controlInput('number', 'filter-upper', this.formState);
      upper.step = 'any';
      upper.setAttribute('aria-label', 'Filter upper bound');
      upperHost.append(labelFor('Filter upper bound', upper));
    }
  }

  private renderColorSection(snapshot: Snapshot, target: FieldTarget): HTMLElement {
    const label = target === 'bus' ? 'Bus' : 'Equipment';
    const title = target === 'bus' ? 'Bus colors' : 'Equipment colors';
    const sectionNode = section(title);
    const scheme = createColorScheme(this.colors, target);
    const fields = definitions(target);
    const field = controlSelect(`${schemeFields(target)}-field`, this.formState);
    field.setAttribute('aria-label', `Color ${target === 'bus' ? 'buses' : 'equipment'} by`);
    option(field, '', 'Default colors');
    for (const item of fields) option(field, item.key, item.label);
    if (![...field.options].some(item => item.value === scheme.field)) scheme.field = '';
    field.value = scheme.field;

    const palette = controlSelect(`${schemeFields(target)}-palette`, this.formState);
    palette.setAttribute('aria-label', `${label} palette`);
    for (const item of COLOR_PALETTES) option(palette, item.id, item.label);
    palette.disabled = !scheme.field;
    if (![...palette.options].some(item => item.value === scheme.palette)) palette.value = palette.options[0]?.value ?? '';
    else palette.value = scheme.palette;

    field.addEventListener('change', () => {
      saveState(field, this.formState);
      const selected = field.value;
      const paletteColorsForValues = paletteColors(scheme.palette);
      scheme.field = selected;
      scheme.colors = {};
      scheme.min = null;
      scheme.max = null;
      if (!selected || isNumericField(target, selected)) {
        scheme.low = paletteColorsForValues[0] ?? scheme.low;
        scheme.high = paletteColorsForValues[paletteColorsForValues.length - 1] ?? scheme.high;
      }
      this.persistColors();
      this.refresh('colors');
    });
    palette.addEventListener('change', () => {
      saveState(palette, this.formState);
      scheme.palette = palette.value;
      const colors = paletteColors(scheme.palette);
      scheme.colors = {};
      scheme.low = colors[0] ?? scheme.low;
      scheme.high = colors[colors.length - 1] ?? scheme.high;
      this.persistColors();
      this.refresh('colors');
    });
    sectionNode.append(labelFor(`Color ${target === 'bus' ? 'buses' : 'equipment'} by`, field), labelFor(`${label} palette`, palette));

    if (scheme.field && fields.find(item => item.key === scheme.field)?.type === 'text') {
      const note = make('p', 'field-controls__session-note');
      note.textContent = 'Custom colors for names and circuits last for this session.';
      sectionNode.append(note);
    }

    if (scheme.field && isNumericField(target, scheme.field)) {
      const gradient = make('div', 'field-controls__gradient');
      for (const [name, key, current] of [
        [`${label} low color`, 'low', scheme.low],
        [`${label} high color`, 'high', scheme.high],
      ] as const) {
        const input = controlInput('color', `${schemeFields(target)}-${key}`, this.formState);
        input.value = validColor(current) ? current : '#4e79a7';
        input.setAttribute('aria-label', name);
        input.addEventListener('input', () => {
          scheme[key] = input.value;
          this.persistColors();
          this.emit('colors');
        });
        gradient.append(labelFor(name, input));
      }
      for (const [name, key] of [[`${label} minimum`, 'min'], [`${label} maximum`, 'max']] as const) {
        const input = controlInput('number', `${schemeFields(target)}-${key}`, this.formState);
        input.step = 'any';
        input.value = scheme[key] === null ? '' : String(scheme[key]);
        input.setAttribute('aria-label', name);
        input.addEventListener('input', () => {
          saveState(input, this.formState);
          const min = key === 'min' ? this.bound(input.value) : scheme.min;
          const max = key === 'max' ? this.bound(input.value) : scheme.max;
          const validation = sectionNode.querySelector<HTMLElement>('.field-controls__validation');
          if (min !== null && max !== null && min > max) {
            if (validation) validation.textContent = `${label} minimum must be less than or equal to maximum.`;
            return;
          }
          if (validation) validation.textContent = '';
          scheme[key] = key === 'min' ? min : max;
          this.persistColors();
          this.emit('colors');
        });
        gradient.append(labelFor(name, input));
      }
      const validation = make('p', 'field-controls__validation');
      validation.setAttribute('role', 'status');
      validation.setAttribute('aria-live', 'polite');
      gradient.append(validation);
      sectionNode.append(gradient);
    } else if (scheme.field) {
      const categoryHost = make('div', 'field-controls__category-editor');
      const category = controlSelect(`${schemeFields(target)}-category`, this.formState);
      category.dataset.categoryTarget = target;
      category.setAttribute('aria-label', `${label} color category`);
      const categories = fieldOptions(snapshot, target, scheme.field)
        .filter((value): value is string | number => value !== null)
        .map(String);
      for (const value of categories) option(category, value, value || '(missing)');
      if (!categories.includes(this.categorySelection[target])) this.categorySelection[target] = categories[0] ?? '';
      category.value = this.categorySelection[target];
      const categoryColor = controlInput('color', `${schemeFields(target)}-category-color`, this.formState);
      const chosen = category.value;
      categoryColor.setAttribute('aria-label', `${label} category color`);
      const chosenIndex = categories.indexOf(chosen);
      const defaultCategoryColor = paletteColors(scheme.palette)[Math.max(0, chosenIndex) % paletteColors(scheme.palette).length] ?? '#4e79a7';
      categoryColor.value = validColor(scheme.colors[chosen] ?? '') ? scheme.colors[chosen] : defaultCategoryColor;
      const apply = make('button');
      apply.type = 'button';
      apply.textContent = `Apply ${target} category color`;
      category.addEventListener('change', () => {
        this.categorySelection[target] = category.value;
        const index = categories.indexOf(category.value);
        const defaults = paletteColors(scheme.palette);
        categoryColor.value = validColor(scheme.colors[category.value] ?? '')
          ? scheme.colors[category.value]
          : defaults[Math.max(0, index) % defaults.length] ?? '#4e79a7';
        saveState(category, this.formState);
      });
      categoryColor.addEventListener('input', () => saveState(categoryColor, this.formState));
      apply.addEventListener('click', () => {
        if (!validColor(categoryColor.value)) return;
        scheme.colors[category.value] = categoryColor.value;
        this.persistColors();
        this.refresh('colors');
      });
      categoryHost.append(labelFor(`${label} color category`, category), labelFor(`${label} category color`, categoryColor), apply);
      sectionNode.append(categoryHost);
    }

    for (const key of ['missing', 'mixed'] as const) {
      const input = controlInput('color', `${schemeFields(target)}-${key}`, this.formState);
      const fieldLabel = key === 'missing' ? `${label} missing color` : `${label} mixed color`;
      input.value = validColor(scheme[key]) ? scheme[key] : (key === 'missing' ? '#b8c0cc' : '#8b5cf6');
      input.setAttribute('aria-label', fieldLabel);
      input.addEventListener('input', () => {
        scheme[key] = input.value;
        this.persistColors();
        this.emit('colors');
      });
      sectionNode.append(labelFor(fieldLabel, input));
    }

    const reset = make('button');
    reset.type = 'button';
    reset.textContent = `Reset ${target} colors`;
    reset.addEventListener('click', () => {
      const defaults = defaultColorSettings()[target];
      this.colors[target] = { ...defaults, colors: { ...defaults.colors } };
      this.persistColors();
      this.refresh('colors');
    });
    sectionNode.append(reset);
    return sectionNode;
  }

  private bound(value: string): number | null {
    if (!value.trim()) return null;
    const number = Number(value);
    return Number.isFinite(number) ? number : null;
  }

  private persistColors(): void {
    saveColorSettings(this.colors);
  }
}
