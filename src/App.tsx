import { useFitWindow } from "./api/window";

export default function App() {
  const ref = useFitWindow<HTMLDivElement>();
  return (
    <div ref={ref} className="popup">
      <p style={{ margin: 0, padding: 16 }}>MultiPath</p>
    </div>
  );
}
