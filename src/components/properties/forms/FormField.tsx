interface FieldProps {
  label: string;
  children: React.ReactNode;
}

export function Field({ label, children }: FieldProps) {
  return (
    <div className="mb-3">
      <label className="block text-xs font-medium text-slate-500 mb-1">
        {label}
      </label>
      {children}
    </div>
  );
}

interface InputProps extends React.InputHTMLAttributes<HTMLInputElement> {}

export function Input(props: InputProps) {
  return (
    <input
      {...props}
      className="w-full px-2 py-1.5 text-xs border border-slate-200 rounded
                 focus:outline-none focus:ring-1 focus:ring-teal-500 focus:border-teal-500
                 bg-white text-slate-700"
    />
  );
}

interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {
  children: React.ReactNode;
}

export function Select({ children, ...props }: SelectProps) {
  return (
    <select
      {...props}
      className="w-full px-2 py-1.5 text-xs border border-slate-200 rounded
                 focus:outline-none focus:ring-1 focus:ring-teal-500 focus:border-teal-500
                 bg-white text-slate-700"
    >
      {children}
    </select>
  );
}

export function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <p className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2 mt-4 first:mt-0">
      {children}
    </p>
  );
}
