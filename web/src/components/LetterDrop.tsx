import { cnm } from '@/utils/style'

// Letters drop in one by one; the full text stays on the wrapper for screen readers.
export default function LetterDrop({ text, className }: { text: string; className?: string }) {
  return (
    <span aria-label={text} className={cnm('drop', className)}>
      {Array.from(text).map((ch, i) => (
        <span key={i} aria-hidden="true" style={{ ['--i' as string]: Math.min(i, 24) }}>
          {ch}
        </span>
      ))}
    </span>
  )
}
