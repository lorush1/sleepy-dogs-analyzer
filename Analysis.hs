module Analysis
  ( scoreEvidence
  , scoreAlibi
  , calculateGuiltProbability
  , findContradictions
  ) where

import Data.Char (toLower)
import Data.List (foldl', isInfixOf)
import Timeline (Alibi(..), Evidence(..), Suspect, TimeRange(..))

scoreEvidence :: Evidence -> Suspect -> Float
scoreEvidence evidence suspect =
  clamp $ base + directionBonus + involvement
  where
    descLower = map toLower (evidenceDescription evidence)
    base = realToFrac (evidenceReliability evidence)
    directionBonus =
      foldr
        (\(kw, bonus) acc -> if kw `isInfixOf` descLower then max acc bonus else acc)
        0
        typeBonuses
    involvement =
      if evidenceSuspectInvolved evidence == suspect
        then 0.1
        else -0.05

typeBonuses :: [(String, Float)]
typeBonuses =
  [ ("weapon", 0.3)
  , ("dna", 0.35)
  , ("fingerprint", 0.25)
  , ("blood", 0.25)
  , ("statement", 0.2)
  , ("confession", 0.4)
  , ("video", 0.3)
  ]

scoreAlibi :: Alibi -> Float
scoreAlibi (Alibi _ (TimeRange start end) witnesses confidence) =
  clamp $ witnessBonus + confWeight + precisionWeight
  where
    witnessBonus = foldl' (\acc _ -> acc + 0.08) 0 witnesses
    confWeight = realToFrac confidence * 0.5
    precision = 1 / (1 + realToFrac (max 1 (end - start)))
    precisionWeight = 0.4 * precision

calculateGuiltProbability :: Suspect -> [Evidence] -> [Alibi] -> Float
calculateGuiltProbability suspect evidence alibis =
  if denominator == 0
    then prior
    else numerator / denominator
  where
    prior = 0.5
    relevantAlibis =
      foldr
        (\alibi acc -> if alibiPerson alibi == suspect then alibi : acc else acc)
        []
        alibis
    evidenceLikelihood =
      foldl'
        (\acc ev -> acc * (1 + min 0.9 (scoreEvidence ev suspect)))
        1
        evidence
    alibiLikelihood =
      foldl'
        (\acc alibi -> acc * (1 + min 0.8 (scoreAlibi alibi)))
        1
        relevantAlibis
    numerator = prior * evidenceLikelihood
    denominator = numerator + (1 - prior) * alibiLikelihood

findContradictions :: [Alibi] -> [Evidence] -> [(Suspect, Suspect, Float)]
findContradictions alibis evidence = foldr collect [] alibis
  where
    collect alibi acc = foldr (check alibi) acc evidence
    check alibi ev acc
      | alibiPerson alibi == evidenceSuspectInvolved ev = acc
      | rangesOverlap (alibiRange alibi) (evidenceRange ev) =
        let strength =
              clamp
                ((scoreAlibi alibi + scoreEvidence ev (evidenceSuspectInvolved ev)) / 2)
         in (alibiPerson alibi, evidenceSuspectInvolved ev, strength) : acc
      | otherwise = acc

rangesOverlap :: TimeRange -> TimeRange -> Bool
rangesOverlap a b = not (endTime a < startTime b || endTime b < startTime a)

evidenceRange :: Evidence -> TimeRange
evidenceRange evidence =
  let ts = evidenceTimestamp evidence
   in TimeRange ts (ts + 1)

clamp :: Float -> Float
clamp x = max 0 (min 1 x)
