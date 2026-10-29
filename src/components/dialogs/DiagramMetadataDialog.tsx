import { useState } from "react";
import { X } from "lucide-react";
import { useDiagramStore } from "../../store/diagramStore";
import { Field, Input } from "../properties/forms/FormField";
import { Modal } from "../../ui/Modal";

interface Props {
  onClose: () => void;
}

export function DiagramMetadataDialog({ onClose }: Props) {
  const metadata = useDiagramStore((s) => s.metadata);
  const updateMetadata = useDiagramStore((s) => s.updateMetadata);

  const [title, setTitle] = useState(metadata.title);
  const [revision, setRevision] = useState(metadata.revision);
  const [company, setCompany] = useState(metadata.company);
  const [description, setDescription] = useState(metadata.description);

  function handleSave() {
    updateMetadata({ title, revision, company, description });
    onClose();
  }

  return (
    <Modal label="Diagram properties" onClose={onClose} className="w-80 p-5">
      <div>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-slate-700">
            Diagram Properties
          </h2>
          <button onClick={onClose} className="text-slate-500 hover:text-slate-700" aria-label="Close">
            <X size={16} />
          </button>
        </div>

        <Field label="Title">
          <Input value={title} onChange={(e) => setTitle(e.target.value)} />
        </Field>
        <Field label="Revision">
          <Input value={revision} onChange={(e) => setRevision(e.target.value)} />
        </Field>
        <Field label="Company">
          <Input value={company} onChange={(e) => setCompany(e.target.value)} />
        </Field>
        <Field label="Description">
          <textarea
            value={description}
            onChange={(e) => setDescription(e.target.value)}
            rows={3}
            className="w-full px-2 py-1.5 text-xs border border-slate-200 rounded
                       focus:outline-none focus:ring-1 focus:ring-teal-500 resize-none"
          />
        </Field>

        <div className="flex gap-2 mt-4">
          <button
            onClick={onClose}
            className="flex-1 py-1.5 text-xs border border-slate-200 rounded text-slate-600 hover:bg-slate-50"
          >
            Cancel
          </button>
          <button
            onClick={handleSave}
            className="flex-1 py-1.5 text-xs rounded bg-teal-700 text-white font-medium hover:bg-teal-800"
          >
            Save
          </button>
        </div>
      </div>
    </Modal>
  );
}
