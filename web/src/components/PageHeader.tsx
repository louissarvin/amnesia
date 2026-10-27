import Eyebrow from '@/components/Eyebrow'
import Headline from '@/components/Headline'

interface PageHeaderProps {
  eyebrow: string
  title: React.ReactNode
  children?: React.ReactNode
}

export default function PageHeader({ eyebrow, title, children }: PageHeaderProps) {
  return (
    <header className="flex flex-col items-center px-5 pt-36 pb-16 text-center md:px-10 md:pt-44 md:pb-20">
      <Eyebrow live>{eyebrow}</Eyebrow>
      <Headline as="h1" className="display mt-6 max-w-4xl text-5xl sm:text-6xl md:text-7xl">
        {title}
      </Headline>
      {children ? <p className="mt-6 max-w-xl text-lg text-muted">{children}</p> : null}
    </header>
  )
}
