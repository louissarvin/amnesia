import { createFileRoute } from '@tanstack/react-router'
import BuiltOn from '@/components/home/BuiltOn'
import Faq from '@/components/home/Faq'
import FooterCta from '@/components/home/FooterCta'
import Forgetting from '@/components/home/Forgetting'
import Hero from '@/components/home/Hero'
import JudgePath from '@/components/home/JudgePath'
import Lifecycle from '@/components/home/Lifecycle'
import LiveCards from '@/components/home/LiveCards'
import Rail from '@/components/home/Rail'
import Redaction from '@/components/home/Redaction'
import Stats from '@/components/home/Stats'

export const Route = createFileRoute('/')({ component: IndexPage })

function IndexPage() {
  return (
    <>
      <Hero />
      <Stats />
      <BuiltOn />
      <JudgePath />
      <Redaction />
      <Lifecycle />
      <LiveCards />
      <Forgetting />
      <Faq />
      <FooterCta />
      <Rail />
    </>
  )
}
